#include <iostream>
#include <fstream>
#include <map>
#include <regex>

#include <apt-pkg/cachefile.h>

#include <capnp/message.h>
#include <capnp/serialize-packed.h>

#include "apt.capnp.h"

static int temp_file();
static void render(int temp, const pkgSrcRecords::Parser *cursor);
static void end();
static std::vector<std::string> keys_in_section(pkgTagSection &sect);
static void render_bin(pkgCache::PkgFileIterator &file, pkgTagSection &sect);

static void fill_keys(const pkgTagSection &sect, const vector<string> &keys,
                      capnp::List<Entry, capnp::Kind::STRUCT>::Builder &builder);

int main(int argc, char *argv[]) {
    if (2 != argc || 0 != strcmp(argv[1], "raw-sources")) {
        fprintf(stderr, "usage: %s raw-sources\n", argv[0]);
        return 2;
    }

    pkgInitConfig(*_config);
    pkgInitSystem(*_config, _system);
    auto *cache_file = new pkgCacheFile();

    const int temp = temp_file();
    pkgSourceList *sources = cache_file->GetSourceList();
    auto *records = new pkgSrcRecords(*sources);
#if 0
    while (const pkgSrcRecords::Parser *cursor = records->Step()) {
        render(temp, cursor);
    }
#endif

    auto pkg_cache = cache_file->GetPkgCache();
    for (auto file = pkg_cache->FileBegin(); file != pkg_cache->FileEnd(); ++file) {
        FileFd fd;
        fd.Open(file.FileName(), FileFd::OpenMode::ReadOnly);
        pkgTagFile tagFile(&fd);
        pkgTagSection sect;
        while (tagFile.Step(sect)) {
            render_bin(file, sect);
        }
    }

    end();

    delete records;
    delete cache_file;

    if (0 != ftruncate(temp, 0)) {
        throw std::runtime_error("couldn't empty temporary file");
    }

    return 0;

}

static void rewind(const int fd) {
    if (0 != lseek(fd, 0, SEEK_SET)) {
        throw std::runtime_error("couldn't rewind file");
    }
}

static void render(const int temp, const pkgSrcRecords::Parser *cursor) {

    ::capnp::MallocMessageBuilder message;
    auto item = message.initRoot<Item>();

    auto root = item.initRawSource();

    root.setPackage(cursor->Package());

    root.setVersion(cursor->Version());

    root.setIndex(cursor->Index().Describe(true));

    {
        std::vector<std::string> raw_binaries;

        {
            // slightly less obviously safe
            const char **b = const_cast<pkgSrcRecords::Parser *>(cursor)->Binaries();
            do {
                raw_binaries.emplace_back(std::string(*b));
            } while (*++b);
        }

        if (raw_binaries.size() > std::numeric_limits<uint>::max()) {
            throw std::runtime_error("can't have more than 'int' binaries");
        }
        auto binaries_builder = root.initBinaries(static_cast<uint>(raw_binaries.size()));
        for (uint i = 0; i < binaries_builder.size(); ++i) {
            binaries_builder.set(i, raw_binaries[i]);
        }
    }

    {
        std::vector<pkgSrcRecords::File2> raw;
        const_cast<pkgSrcRecords::Parser *>(cursor)->Files2(raw);

        if (raw.size() > std::numeric_limits<uint>::max()) {
            throw std::runtime_error("can't have more than 'int' files");
        }

        auto files = root.initFiles(static_cast<uint>(raw.size()));

        uint pos = 0;
        for (auto &file2 : raw) {
            std::string name = file2.Path;

            files[pos].setName(name);
            files[pos].setSize(file2.FileSize);
            const HashString *const md5 = file2.Hashes.find("MD5Sum");
            if (md5) {
                files[pos].setMd5(md5->HashValue());
            }

            const HashString *const sha1 = file2.Hashes.find("SHA1");
            if (sha1) {
                files[pos].setSha1(sha1->HashValue());
            }

            const HashString *const sha256 = file2.Hashes.find("SHA256");
            if (sha256) {
                files[pos].setSha256(sha256->HashValue());
            }

            const HashString *const sha512 = file2.Hashes.find("Sha512");
            if (sha512) {
                files[pos].setSha512(sha512->HashValue());
            }

            ++pos;
        }
    }

    {
        // This is so dumb. Can't even get access to the parsed data,
        // so we have to re-serialise and re-parse it.

        // It's like being stabbed repeatedly in the face.

        // No idea why this is a const method; pretty angry.
        std::string body = const_cast<pkgSrcRecords::Parser *>(cursor)->AsStr();
        body.push_back('\n');

        rewind(temp);

        size_t written = 0;
        const char *data = body.c_str();

        while (true) {
            const ssize_t to_write = body.size() - written;
            ssize_t wrote = write(temp, data + written, to_write);
            if (wrote == to_write) {
                break;
            }
            if (wrote <= 0) {
                if (EAGAIN == errno) {
                    continue;
                }

                throw std::runtime_error("couldn't write file");
            }
            written += wrote;
        }

        rewind(temp);
    }

    {
        FileFd fd;
        fd.OpenDescriptor(temp, FileFd::OpenMode::ReadOnly, FileFd::CompressMode::None);
        pkgTagFile a(&fd);
        pkgTagSection sect;
        if (!a.Step(sect)) {
            throw std::runtime_error("didn't manage to load a record");
        }

        auto keys = keys_in_section(sect);

        auto builder = root.initEntries(static_cast<uint>(keys.size()));
        fill_keys(sect, keys, builder);
    }

    ::capnp::writeMessageToFd(1, message);
}

static void fill_keys(const pkgTagSection &sect, const vector<string> &keys,
               capnp::List<Entry, capnp::Kind::STRUCT>::Builder &builder) {
    uint pos = 0;
    for (const string &key : keys) {
            auto entry = builder[pos++];
            entry.setKey(key);
            entry.setValue(sect.FindS(key.c_str()));
        }
}

static void render_bin(pkgCache::PkgFileIterator &file, pkgTagSection &sect) {
    ::capnp::MallocMessageBuilder message;
    auto item = message.initRoot<Item>();
    auto root = item.initRawBinary();

    auto index = root.initIndex();
#define set(X) if (file.X() && *file.X()) { index.set##X(file.X()); }
    set(Archive);
    set(Version);
    set(Origin);
    set(Codename);
    set(Label);
    set(Site);
    set(Component);
#undef set

    if (file.Architecture() && *file.Architecture()) { index.setArch(file.Architecture()); }
    if (file.IndexType() && *file.IndexType()) { index.setType(file.IndexType()); }

    auto keys = keys_in_section(sect);
    auto builder = root.initEntries(keys.size());
    fill_keys(sect, keys, builder);

    ::capnp::writeMessageToFd(1, message);
}

static std::vector<std::string> keys_in_section(pkgTagSection &sect) {
    std::vector<std::string> keys;
    keys.reserve(sect.Count());

    {
        for (unsigned int i = 0; i < sect.Count(); ++i) {
            const char *start;
            const char *end;
            sect.Get(start, end, i);

            const char *colon = strchr(start, ':');
            if (!colon || colon >= end) {
                throw std::runtime_error("couldn't find colon in field: " + std::string(start, end));
            }

            auto key = std::string(start, colon);
            if (key != "Package" &&
                key != "Version" &&
                key != "Binary" &&
                key != "Files" &&
                key != "Checksums-Sha1" &&
                key != "Checksums-Sha256" &&
                key != "Checksums-Sha512") {
                keys.emplace_back(key);
            }
        }
    }
    if (keys.size() > std::numeric_limits<uint>::max()) {
        throw std::runtime_error("can't have more than 'int' entries");
    }

    return keys;
}

static void end() {
    ::capnp::MallocMessageBuilder message;
    auto item = message.initRoot<Item>();
    item.setEnd();
    ::capnp::writeMessageToFd(1, message);
}

static int temp_file() {
    constexpr size_t len = 30;
    char buf[len] = {};
    snprintf(buf, len - 1, "/tmp/apt-dump.XXXXXX");
    int fd = mkstemp(buf);

    if (-1 == fd) {
        throw std::runtime_error("couldn't create temporary file");
    }

    return fd;
}
