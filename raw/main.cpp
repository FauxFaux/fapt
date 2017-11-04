#include <cassert>
#include <string>
#include <limits>
#include <vector>

#include <apt-pkg/cachefile.h>

#include <capnp/message.h>
#include <capnp/serialize-packed.h>

#include "apt.capnp.h"

struct IndexFileData {
    std::string Archive;
    std::string Version;
    std::string Origin;
    std::string Codename;
    std::string Label;
    std::string Site;
    std::string Component;
    std::string Arch;
    std::string Type;
};

static void render_index(const IndexFileData &index_file);
static void render_whole_file(const char *name, bool src);
static void render_end();

static void render_src(const pkgSourceList *apt_sources_list);
static void render_bin(pkgCache *pkg_cache);

static std::vector<std::string> keys_in_section(pkgTagSection &sect);


int main(int argc, char *argv[]) {
    if (2 != argc || 0 != strcmp(argv[1], "raw-sources")) {
        fprintf(stderr, "usage: %s raw-sources\n", argv[0]);
        return 2;
    }

    pkgInitConfig(*_config);
    pkgInitSystem(*_config, _system);
    auto *cache_file = new pkgCacheFile();

    pkgSourceList *apt_sources_list = cache_file->GetSourceList();
    render_src(apt_sources_list);

    auto pkg_cache = cache_file->GetPkgCache();
    render_bin(pkg_cache);

    render_end();

    delete cache_file;

    return 0;

}

static void render_src(const pkgSourceList *apt_sources_list) {// like "pkgSrcRecords::pkgSrcRecords(pkgSourceList &List) {"
    // apt_source_list_entry is something like "all the lines for a url and distribution (sid)";
    // e.g. "deb http://foo sid main lol; deb-src http://foo sid main lol"
    // .. which appears to be what a Release file contains. Right.
    for (metaIndex *apt_sources_list_entry : *apt_sources_list) {
        for (auto const &target : apt_sources_list_entry->GetIndexTargets()) {
            // like "std::vector<pkgIndexFile *> *debReleaseIndex::GetIndexFiles()"
            const string createdBy = target.Option(IndexTarget::CREATED_BY);

            auto filename = target.Option(IndexTarget::FILENAME);
            printf("%s\n", filename.c_str());

            if ("Sources" != createdBy) {
                continue;
            }

            IndexFileData index_file;

            // TODO: Only two missing? Miracle.
//            index_file.Archive = apt_sources_list_entry->ArchiveURI()
            index_file.Version = apt_sources_list_entry->GetVersion();
            index_file.Origin = apt_sources_list_entry->GetOrigin();
            index_file.Codename = apt_sources_list_entry->GetCodename();
            index_file.Label = apt_sources_list_entry->GetLabel();
            index_file.Site = target.Option(IndexTarget::SITE);
            index_file.Component = target.Option(IndexTarget::COMPONENT);
//            index_file.Arch = target.Option(IndexTarget::);
            index_file.Type = apt_sources_list_entry->GetType();

            render_index(index_file);
            render_whole_file(filename.c_str(), true);
        }
    }
}

static void render_bin(pkgCache *pkg_cache) {
    for (auto file = pkg_cache->FileBegin(); file != pkg_cache->FileEnd(); ++file) {
        IndexFileData index_file = {};
#define set(X) if (file.X() && *file.X()) { index_file.X = file.X(); }
        set(Archive);
        set(Version);
        set(Origin);
        set(Codename);
        set(Label);
        set(Site);
        set(Component);
#undef set

        if (file.Architecture() && *file.Architecture()) { index_file.Arch = file.Architecture(); }
        if (file.IndexType() && *file.IndexType()) { index_file.Type = file.IndexType(); }

        render_index(index_file);
        render_whole_file(file.FileName(), false);
    }
}

void render_index(const IndexFileData &index_file) {
    capnp::MallocMessageBuilder message;
    auto item = message.initRoot<Item>();
    auto builder = item.initIndex();

#define set(X) if (!index_file.X.empty()) { builder.set##X(index_file.X); }
    set(Archive);
    set(Version);
    set(Origin);
    set(Codename);
    set(Label);
    set(Site);
    set(Component);
    set(Arch);
    set(Type);
#undef set

    writeMessageToFd(1, message);
}

void render_whole_file(const char *name, bool src) {
    FileFd fd;
    fd.Open(name, FileFd::ReadOnly);
    pkgTagFile tagFile(&fd);
    pkgTagSection sect;
    while (tagFile.Step(sect)) {
        capnp::MallocMessageBuilder message;
        auto item = message.initRoot<Item>();
        auto root = item.initRaw();

        root.setType(src ? RawPackageType::SOURCE : RawPackageType::BINARY);

        auto keys = keys_in_section(sect);
        assert(keys.size() < std::numeric_limits<unsigned int>::max());
        auto builder = root.initEntries(keys.size());

        uint pos = 0;
        for (const string &key : keys) {
            auto entry = builder[pos++];
            entry.setKey(key);
            entry.setValue(sect.FindS(key.c_str()));
        }

        writeMessageToFd(1, message);
    }
}

static void render_end() {
    ::capnp::MallocMessageBuilder message;
    auto item = message.initRoot<Item>();
    item.setEnd();
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
            keys.emplace_back(key);
        }
    }
    if (keys.size() > std::numeric_limits<uint>::max()) {
        throw std::runtime_error("can't have more than 'int' entries");
    }

    return keys;
}
