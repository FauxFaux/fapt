#include <iostream>
#include <fstream>
#include <map>
#include <string>

#include <apt-pkg/cachefile.h>
#include <apt-pkg/pkgcache.h>
#include <apt-pkg/version.h>

#include <capnp/message.h>
#include <capnp/serialize-packed.h>
#include <sstream>
#include <cstdlib>

#include "apt.capnp.h"

struct FileHash {
    uint64_t size;
    std::string checksum;
};

using map_t = std::map<std::string, std::string>;
using files_t = std::map<std::string, FileHash>;

static std::string temp_name();
static map_t load_single(const std::string &body);
static std::string take_mandatory(map_t &map, const std::string &key);
static std::string take_optional(map_t &map, const std::string &key);
static std::vector<std::string> split(const std::string &s, char delim);
static files_t parse_files(const std::string &data);

int main() {
    pkgInitConfig(*_config);
    pkgInitSystem(*_config, _system);

    auto *cache_file = new pkgCacheFile();
    pkgSourceList *sources = cache_file->GetSourceList();
    auto *records = new pkgSrcRecords(*sources);
    const pkgSrcRecords::Parser *cursor = records->Step();

    // This is so dumb. Can't even get access to the parsed data,
    // so we have to re-serialise and re-parse it.

    // It's like being stabbed repeatedly in the face.

    // No idea why this is a const method; pretty angry.
    auto body = const_cast<pkgSrcRecords::Parser *>(cursor)->AsStr();

    std::map<std::string, std::string> val = load_single(body);

    for (auto& kv : val) {
        std::cerr << kv.first << " -> " << kv.second << std::endl;
    }

    ::capnp::MallocMessageBuilder message;

    auto root = message.initRoot<Source>();
    root.setPackage(cursor->Package());
    root.setVersion(cursor->Version());

    root.setDirectory(take_mandatory(val, "Directory"));
    root.setHomepage(take_mandatory(val, "Homepage"));
    root.setSection(take_mandatory(val, "Section"));

    // Maintainer will be deleted, and replaced with Original-Maintainer, even in the file.
    root.setMaintainer(cursor->Maintainer());

    root.setStandards(take_mandatory(val, "Standards-Version"));

    {
        auto arch = root.initArch(1);
        // TODO: split
        arch.set(0, take_mandatory(val, "Architecture"));
    }

    {
#if 0
        // TODO: check raw_binaries against our parse of Package-List
        std::vector<std::string> raw_binaries;

        {
            // slightly less obviously safe
            const char **b = const_cast<pkgSrcRecords::Parser *>(cursor)->Binaries();
            do {
                raw_binaries.emplace_back(std::string(*b));
            } while (*++b);
        }
#endif

        // TODO: sorting?

        std::vector<std::string> packages = split(take_mandatory(val, "Package-List"), '\n');
        if (packages.size() > std::numeric_limits<uint>::max()) {
            throw std::runtime_error("can't have more than 'int' binaries");
        }

        auto binaries = root.initBinaries(static_cast<unsigned int>(packages.size()));
        for (uint i = 0; i < binaries.size(); ++i) {
            std::vector<std::string> parts = split(packages[i], ' ');
            if (5 != parts.size()) {
                throw std::runtime_error("failed to parse Package-List");
            }

            binaries[i].setName(parts[0]);
            binaries[i].setStyle(parts[1]);
            binaries[i].setSection(parts[2]);
            binaries[i].setPriority(parts[3]);
            binaries[i].setArchSpec(parts[4]);
        }
    }

    // TODO: build deps

    {
        files_t md5 = parse_files(take_mandatory(val, "Files"));

        if (md5.size() > std::numeric_limits<uint>::max()) {
            throw std::runtime_error("can't have more than 'int' files");
        }

        files_t sha1 = parse_files(take_optional(val, "Checksums-Sha1"));
        files_t sha256 = parse_files(take_optional(val, "Checksums-Sha256"));
        files_t sha512 = parse_files(take_optional(val, "Checksums-Sha512"));

        auto files = root.initFiles(static_cast<uint>(md5.size()));

        uint pos = 0;
        for (auto &kv : md5) {
            std::string name = kv.first;

            files[pos].setName(name);
            files[pos].setSize(kv.second.size);
            files[pos].setMd5(kv.second.checksum);

            if (!sha1.empty()) {
                auto it = sha1.find(name);
                if (sha1.end() == it) {
                    throw std::runtime_error("there are sha1s but not for " + name);
                }
                files[pos].setSha1(it->second.checksum);
            }

            if (!sha256.empty()) {
                auto it = sha256.find(name);
                if (sha256.end() == it) {
                    throw std::runtime_error("there are sha256s but not for " + name);
                }
                files[pos].setSha256(it->second.checksum);
            }

            if (!sha512.empty()) {
                auto it = sha512.find(name);
                if (sha512.end() == it) {
                    throw std::runtime_error("there are sha512s but not for " + name);
                }
                files[pos].setSha512(it->second.checksum);
            }

            ++pos;
        }
    }

    {
        map_t vcses;
        for (auto &tag : {"Browser", "Arch", "Bzr", "Cvs", "Darcs", "Git", "Hg", "Mtn", "Svn"}) {
            auto text = take_optional(val, std::string("Vcs-") + tag);
            if (text.empty()) {
                continue;
            }

            vcses[tag] = text;
        }

        auto vcs = root.initVcs(static_cast<uint>(vcses.size()));
        uint pos = 0;

        for (auto &kv : vcses) {
            vcs[pos].setDescription(kv.second);
            auto type = vcs[pos].initType();
            if ("Browser" == kv.first) {
                type.setBrowser();
            } else if ("Arch" == kv.first) {
                type.setArch();
            } else if ("Bzr" == kv.first) {
                type.setBzr();
            } else if ("Cvs" == kv.first) {
                type.setCvs();
            } else if ("Darcs" == kv.first) {
                type.setDarcs();
            } else if ("Git" == kv.first) {
                type.setGit();
            } else if ("Hg" == kv.first) {
                type.setHg();
            } else if ("Mtn" == kv.first) {
                type.setMtn();
            } else if ("Svn" == kv.first) {
                type.setSvn();
            } else {
                throw std::runtime_error("unreachable code");
            }

            ++pos;
        }
    }

    {
        std::string format = take_mandatory(val, "Format");

        if ("3.0 (quilt)" == format) {
            root.initFormat().setQuilt3dot0();
        } else if ("3.0 (native)" == format) {
            root.initFormat().setNative3dot0();
        } else if ("1.0" == format) {
            root.initFormat().setOriginal();
        } else if ("3.0 (git)" == format) {
            root.initFormat().setGit3dot0();
        } else {
            throw std::runtime_error("unrecognised format: " + format);
        }
    }

    ::capnp::writeMessageToFd(1, message);

    delete records;
    delete cache_file;

    return 0;
}

static map_t load_single(const std::string &body) {
    const string filename = temp_name();

    {
        std::ofstream o(filename);
        o << body;
    }

    map_t ret;

    {
        FileFd fd;
        fd.Open(filename, FileFd::OpenMode::ReadOnly);
        pkgTagFile a(&fd);
        pkgTagSection sect;
        a.Step(sect);

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

            ret[name] = value;
        }
    }

    if (0 != std::remove(filename.c_str())) {
        throw std::runtime_error("couldn't remove temporary file");
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

static std::string take_mandatory(map_t &map, const std::string &key) {
    auto it = map.find(key);
    if (map.end() == it) {
        throw std::runtime_error("mandatory key " + key + " is missing");
    }

    std::string ret = it->second;
    map.erase(it);

    return ret;
}


static std::string take_optional(map_t &map, const std::string &key) {
    auto it = map.find(key);
    if (map.end() == it) {
        return "";
    }

    std::string ret = it->second;
    map.erase(it);

    return ret;
}

static files_t parse_files(const std::string &data) {
    files_t ret;
    for (auto &line : split(data, '\n')) {
        auto parts = split(line, ' ');
        if (3 != parts.size()) {
            throw std::runtime_error("invalid Files/checksum section");
        }

        if (ret.find(parts[2]) != ret.end()) {
            throw std::runtime_error("invalid Files/checksum section: duplicate name");
        }

        FileHash fh;
        fh.checksum = parts[0];
        fh.size = std::stoull(parts[1]);

        ret[parts[2]] = fh;
    }

    return ret;
}

// if only C++ was a language people actually wrote code in

static inline void ltrim(std::string &s) {
    s.erase(s.begin(), std::find_if(s.begin(), s.end(), [](int ch) {
        return !std::isspace(ch);
    }));
}

static inline void rtrim(std::string &s) {
    s.erase(std::find_if(s.rbegin(), s.rend(), [](int ch) {
        return !std::isspace(ch);
    }).base(), s.end());
}

static std::vector<std::string> split(const std::string &s, char delim) {
    std::vector<std::string> elems;
    std::stringstream ss;
    ss.str(s);
    std::string item;

    while (std::getline(ss, item, delim)) {
        ltrim(item);
        rtrim(item);
        elems.push_back(item);
    }

    return elems;
}

