#include <iostream>
#include <fstream>
#include <map>
#include <string>
#include <regex>

#include <apt-pkg/cachefile.h>
#include <apt-pkg/pkgcache.h>
#include <apt-pkg/version.h>

#include <capnp/message.h>
#include <capnp/serialize-packed.h>
#include <sstream>
#include <cstdlib>

#include "apt.capnp.h"

struct SingleDep {
    std::string package;
    std::string arch;
    std::vector<std::pair<std::string, std::string>> version_constraints;
    std::vector<std::string> arch_constraints;
    std::vector<std::string> stage_constraints;
};

using map_t = std::map<std::string, std::string>;

static std::string temp_name();
static map_t load_single(const std::string &body);
static std::string take_mandatory(map_t &map, const std::string &key);
static std::string take_optional(map_t &map, const std::string &key);
static std::vector<std::string> split(const std::string &s, char delim);
static void render(const pkgSrcRecords::Parser *cursor);

static std::vector<std::vector<SingleDep>> parse_deps(std::string deps);

template<typename T> void set_priority(T& thing, const std::string &from) {
    if ("required" == from) {
        thing.setRequired();
    } else if ("important" == from) {
        thing.setImportant();
    } else if ("standard" == from) {
        thing.setStandard();
    } else if ("optional" == from) {
        thing.setOptional();
    } else if ("extra" == from) {
        thing.setExtra();
    } else if ("source" == from) {
        thing.setSource();
    } else {
        throw std::runtime_error("unrecognised priority: " + from);
    }
}

int main() {
    pkgInitConfig(*_config);
    pkgInitSystem(*_config, _system);

    auto *cache_file = new pkgCacheFile();
    pkgSourceList *sources = cache_file->GetSourceList();
    auto *records = new pkgSrcRecords(*sources);
    while (const pkgSrcRecords::Parser *cursor = records->Step()) {
        render(cursor);
    }

    delete records;
    delete cache_file;

    return 0;

}

static void render(const pkgSrcRecords::Parser *cursor) {
    // This is so dumb. Can't even get access to the parsed data,
    // so we have to re-serialise and re-parse it.

    // It's like being stabbed repeatedly in the face.

    // No idea why this is a const method; pretty angry.
    auto body = const_cast<pkgSrcRecords::Parser *>(cursor)->AsStr();

    std::map<std::string, std::string> val = load_single(body);

#if 0
    for (auto& kv : val) {
        std::cerr << kv.first << " -> " << kv.second << std::endl;
    }
#endif

    ::capnp::MallocMessageBuilder message;

    auto root = message.initRoot<Source>();

    root.setPackage(cursor->Package());
    val.erase("Package");
    val.erase("Source");

    root.setVersion(cursor->Version());
    val.erase("Version");

    root.setDirectory(take_mandatory(val, "Directory"));
    {
        const std::string homepage = take_optional(val, "Homepage");
        if (!homepage.empty()) {
            root.setHomepage(homepage);
        }
    }

    root.setSection(take_mandatory(val, "Section"));

    root.setMaintainer(take_mandatory(val, "Maintainer"));
    {
        const std::string orig = take_optional(val, "Original-Maintainer");
        if (!orig.empty()) {
            root.setOrigMaint(orig);
        }
    }


    {
        const std::string str = take_optional(val, "Priority");
        if (!str.empty()) {
            Priority::Builder priority = root.initPriority();
            set_priority(priority, str);
        }
    }

    {
        const std::string str = take_optional(val, "Standards-Version");
        if (!str.empty()) {
            root.setStandards(str);
        }
    }

    {
        auto arch = root.initArch(1);
        // TODO: split
        arch.set(0, take_mandatory(val, "Architecture"));
    }

    {
        // TODO: check raw_binaries against our parse of Package-List
        std::vector<std::string> raw_binaries;

        {
            // slightly less obviously safe
            const char **b = const_cast<pkgSrcRecords::Parser *>(cursor)->Binaries();
            do {
                raw_binaries.emplace_back(std::string(*b));
            } while (*++b);
        }
        val.erase("Binary");

        // TODO: sorting?

        std::string list = take_optional(val, "Package-List");
        if (!list.empty()) {
            std::vector<std::string> packages = split(list, '\n');
            if (packages.size() > std::numeric_limits<uint>::max()) {
                throw std::runtime_error("can't have more than 'int' binaries");
            }

            auto binaries = root.initBinaries(static_cast<unsigned int>(packages.size()));
            for (uint i = 0; i < binaries.size(); ++i) {
                std::vector<std::string> parts = split(packages[i], ' ');
                if (parts.size() < 4) {
                    throw std::runtime_error("failed to parse Package-List");
                }

                binaries[i].setName(parts[0]);
                binaries[i].setStyle(parts[1]);
                binaries[i].setSection(parts[2]);
                Priority::Builder priority = binaries[i].initPriority();
                set_priority(priority, parts[3]);
                auto extras = binaries[i].initExtras(parts.size() - 4);
                for (uint j = 0; j < extras.size(); ++j) {
                    extras.set(j, parts[j + 4]);
                }
            }
        } else {
            auto binaries = root.initBinaries(raw_binaries.size());
            for (uint i = 0; i < binaries.size(); ++i) {
                binaries[i].setName(raw_binaries[i]);
            }
        }
    }

    {
        auto deps = parse_deps(take_optional(val, "Build-Depends"));
        auto deps_builder = root.initBuildDep(deps.size());
        for (uint i = 0; i < deps_builder.size(); ++i) {
            auto alt = deps[i];
            auto alt_builder = deps_builder[i].initAlternate(alt.size());
            for (uint j = 0; j < alt_builder.size(); ++j) {
                auto dep = alt[j];
                alt_builder[j].setPackage(dep.package);
                if (!dep.arch.empty()) {
                    alt_builder[j].setArch(dep.arch);
                }
                if (dep.version_constraints.empty()) {
                    continue;
                }
                auto version_builder = alt_builder[j].initVersionConstraints(dep.version_constraints.size());
                for (uint k = 0; k < version_builder.size(); ++k) {
                    version_builder[k].setVersion(dep.version_constraints[k].first);
                    std::string op = dep.version_constraints[k].second;
                    if ("<=" == op) {
                        version_builder[k].initOperator().setLe();
                    } else if (">=" == op) {
                        version_builder[k].initOperator().setGe();
                    } else if ("<<" == op) {
                        version_builder[k].initOperator().setLt();
                    } else if (">>" == op) {
                        version_builder[k].initOperator().setGt();
                    } else {
                        throw std::runtime_error("unknown operator '" + op + "'");
                    }
                }
            }
        }
    }

    // TODO: other types of build dep

    {
        std::vector<pkgSrcRecords::File2> raw;
        const_cast<pkgSrcRecords::Parser *>(cursor)->Files2(raw);

        val.erase("Files");
        val.erase("Checksums-Sha1");
        val.erase("Checksums-Sha256");
        val.erase("Checksums-Sha512");

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

    if (!val.empty()) {
        std::cerr << "Some values not consumed:" << std::endl;
        for (auto &kv : val) {
            std::cerr << " * " << kv.first << std::endl;
        }
    }

    ::capnp::writeMessageToFd(1, message);
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

static std::vector<std::vector<SingleDep>> parse_deps(std::string deps) {
    std::vector<std::vector<SingleDep>> ret;
//    const std::string r_version = R"gex()gex";
    const std::string r_version = R"(\(([<=>]+)\s*([a-zA-Z0-9.~+:-]+)\))";
    const std::string r_package = R"(([a-z0-9.+-]+)(:[a-z0-9]+)?((?:\s*)"
                                  + r_version
                                  + ")*)"
                                  // [linux-any]
                                  + R"((?:\s*\[([!a-z0-9 -]+)\])?)"
                                    // <!nocheck> and <!foo> <!bar>
                                  + R"((?:\s*<([!a-z0-9. ]+)>)*)"
                                  + "\\s*";
    const std::string r_alternate = "^\\s*,?\\s*" + r_package + R"((?:\s*\|\s*)" + r_package + ")*";

    std::regex alt_regex(r_alternate, std::regex_constants::ECMAScript);
    std::regex pkg_regex(r_package, std::regex_constants::ECMAScript);
    std::regex version_regex(r_version, std::regex_constants::ECMAScript);

    std::smatch alternate_expression;
    while (std::regex_search(deps, alternate_expression, alt_regex)) {
        std::vector<SingleDep> this_alt;
        const std::string whole_expr = alternate_expression.str();
        for (auto package_expression = std::sregex_iterator(whole_expr.cbegin(), whole_expr.cend(), pkg_regex);
             package_expression != std::sregex_iterator();
             ++package_expression) {

            SingleDep dep;

            auto y = package_expression->cbegin();
            dep.package = (++y)->str();
            dep.arch = (++y)->str();
            std::string versions = (++y)->str();
            ++y; // last matched op
            ++y; // last matched version
            std::string arch = (++y)->str();
            std::string cond = (++y)->str();

            // TODO: arch, cond

            for (auto version = std::sregex_iterator(versions.cbegin(), versions.cend(), version_regex);
                 version != std::sregex_iterator();
                 ++version) {
                auto z = version->cbegin();
                std::string op = (++z)->str();
                std::string ver = (++z)->str();
                dep.version_constraints.push_back({ver, op});
            }

            this_alt.push_back(dep);
        }
        ret.push_back(this_alt);
        deps = deps.substr(alternate_expression.length());
    }

    if (!deps.empty()) {
        throw std::runtime_error("didn't fully consume deps string: " + deps);
    }

    return ret;
}
