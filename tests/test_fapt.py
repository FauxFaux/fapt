import pytest

from fapt import commands, sources_list, system


def test_instantiation():
    system.System.cache_only()


@pytest.fixture
def system_instance():
    return system.System.cache_only()


@pytest.fixture
def entry_instance():
    root_url = "http://ca.archive.ubuntu.com/ubuntu/"
    dist = "focal"
    components = ["main", "restricted"]

    return sources_list.Entry(False, root_url, dist, components, None)


def test_add_sources_entries(entry_instance, system_instance):
    system_instance.add_sources_entries([])
    system_instance.add_sources_entries([entry_instance])


class TestUpdate:
    def test_noop(self, system_instance):
        system_instance.update()

    def test_with_entry(self, entry_instance, system_instance):
        commands.add_builtin_keys(system_instance)
        system_instance.add_sources_entries([entry_instance])
        system_instance.update()


def test_listings(entry_instance, system_instance):
    commands.add_builtin_keys(system_instance)
    system_instance.add_sources_entries([entry_instance])
    system_instance.set_arches(["amd64"])
    system_instance.update()
    assert len(system_instance.listings()) > 0
    for dl in system_instance.listings():
        assert [entry_instance] == dl.release.sources_entries
