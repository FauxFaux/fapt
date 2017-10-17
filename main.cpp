#include <iostream>
#include <fstream>
#include <map>
#include <regex>

#include <apt-pkg/cachefile.h>

#include <capnp/message.h>
#include <capnp/serialize-packed.h>

#include "apt.capnp.h"

using map_t = std::vector<std::pair<std::string, std::string>>;

static int temp_file();
static void render(int temp, const pkgSrcRecords::Parser *cursor);

int main() {
    pkgInitConfig(*_config);
    pkgInitSystem(*_config, _system);

    const int temp = temp_file();

    auto *cache_file = new pkgCacheFile();
    pkgSourceList *sources = cache_file->GetSourceList();
    auto *records = new pkgSrcRecords(*sources);
    while (const pkgSrcRecords::Parser *cursor = records->Step()) {
        render(temp, cursor);
    }

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

    auto root = message.initRoot<RawSource>();

    root.setPackage(cursor->Package());

    root.setVersion(cursor->Version());

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
        const std::string body = const_cast<pkgSrcRecords::Parser *>(cursor)->AsStr();

        if (0 != ftruncate(temp, 0)) {
            throw std::runtime_error("couldn't truncate temp file");
        }
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

    map_t ret;

    {
        FileFd fd;
        fd.OpenDescriptor(temp, FileFd::OpenMode::ReadOnly, FileFd::CompressMode::None);
        pkgTagFile a(&fd);
        pkgTagSection sect;
        if (!a.Step(sect)) {
            throw std::runtime_error("didn't manage to load a record");
        }

        std::vector<std::string> keys;
        keys.reserve(sect.Count() - 4);

        {
            for (unsigned int i = 0; i < sect.Count(); ++i) {
                const char *start;
                const char *end;
                sect.Get(start, end, i);

                const char *colon = strchr(start, ':');
                if (!colon || colon >= end) {
                    std::cerr << i << std::endl;
                    throw std::runtime_error("couldn't find colon in field: " + std::string(start, end));
                }

                auto key = std::string(start, colon);
                if (key != "Package" &&
                    key != "Version" &&
                    key != "Binaries" &&
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

        auto builder = root.initEntries(static_cast<uint>(keys.size()));
        uint pos = 0;
        for (const std::string &key : keys) {
            auto entry = builder[pos++];
            entry.setKey(key);
            entry.setValue(sect.FindS(key.c_str()));
        }
    }

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
