#include <iostream>
#include <fstream>
#include <map>
#include <regex>

#include <apt-pkg/cachefile.h>

#include <capnp/message.h>
#include <capnp/serialize-packed.h>

#include "apt.capnp.h"

using map_t = std::vector<std::pair<std::string, std::string>>;

static std::string temp_name();
static map_t load_single(const std::string &temp, const std::string &body);
static void render(const std::string &temp, const pkgSrcRecords::Parser *cursor);
static void erase_first(map_t &from, const char *val);

int main() {
    pkgInitConfig(*_config);
    pkgInitSystem(*_config, _system);

    const std::string temp = temp_name();

    auto *cache_file = new pkgCacheFile();
    pkgSourceList *sources = cache_file->GetSourceList();
    auto *records = new pkgSrcRecords(*sources);
    while (const pkgSrcRecords::Parser *cursor = records->Step()) {
        render(temp, cursor);
    }

    delete records;
    delete cache_file;

    if (0 != std::remove(temp.c_str())) {
        throw std::runtime_error("couldn't remove temporary file");
    }

    return 0;

}

static void render(const std::string &temp, const pkgSrcRecords::Parser *cursor) {
    // This is so dumb. Can't even get access to the parsed data,
    // so we have to re-serialise and re-parse it.

    // It's like being stabbed repeatedly in the face.

    // No idea why this is a const method; pretty angry.
    auto body = const_cast<pkgSrcRecords::Parser *>(cursor)->AsStr();

    map_t val = load_single(temp, body);

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

    erase_first(val, "Package");
    erase_first(val, "Version");
    erase_first(val, "Binaries");
    erase_first(val, "Files");
    erase_first(val, "Checksums-Sha1");
    erase_first(val, "Checksums-Sha256");
    erase_first(val, "Checksums-Sha512");

    if (val.size() > std::numeric_limits<uint>::max()) {
        throw std::runtime_error("can't have more than 'int' entries");
    }

    auto entries_builder = root.initEntries(static_cast<uint>(val.size()));
    for (uint i = 0; i < entries_builder.size(); ++i) {
        entries_builder[i].setKey(val[i].first);
        entries_builder[i].setValue(val[i].second);
    }

    ::capnp::writeMessageToFd(1, message);
}

static map_t load_single(const std::string &temp, const std::string &body) {

    {
        std::ofstream o(temp);
        o << body;
    }

    map_t ret;

    {
        FileFd fd;
        fd.Open(temp, FileFd::OpenMode::ReadOnly);
        pkgTagFile a(&fd);
        pkgTagSection sect;
        if (!a.Step(sect)) {
            throw std::runtime_error("didn't manage to load a record");
        }

        for (unsigned int i = 0; i < sect.Count(); ++i) {
            const char *start;
            const char *end;
            sect.Get(start, end, i);
            const std::string whole_field(start, end);
            const size_t colon = whole_field.find(':');
            if (std::string::npos == colon) {
                throw std::runtime_error("no colon in tag: " + whole_field);
            }

            std::string name = whole_field.substr(0, colon);
            std::string value = sect.FindS(name.c_str());

            ret.emplace_back(name, value);
        }
    }

    return ret;
}

static std::string temp_name() {
    constexpr size_t len = 30;
    char buf[len] = {};
    snprintf(buf, len - 1, "/tmp/apt_dump.XXXXXX");

    int fd = mkstemp(buf);

    if (-1 == fd) {
        throw std::runtime_error("couldn't create temporary file");
    }

    if (-1 == close(fd)) {
        throw std::runtime_error("couldn't close temporary file");
    }

    return std::string(buf);
}

static void erase_first(map_t &from, const char *val) {
    auto it = std::find_if(from.cbegin(), from.cend(), [&](auto x) { return x.first == val; });
    if (it == from.cend()) {
        return;
    }

    from.erase(it);
}
