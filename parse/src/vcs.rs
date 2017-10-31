use std::collections::HashMap;

use apt_capnp;
use as_u32;
use errors::*;

#[derive(Copy, Clone, Debug)]
enum Vcs {
    Arch,
    Browser,
    Bzr,
    Cvs,
    Darcs,
    Git,
    Hg,
    Mtn,
    Svn,
}

#[derive(Copy, Clone, Debug)]
enum Tag {
    Vcs,
    Orig,
    Debian,
    Upstream,
}

#[derive(Clone, Debug)]
struct Entry {
    description: String,
    vcs: Vcs,
    tag: Tag,
}

impl Entry {
    pub fn new(description: &str, vcs: &Vcs, tag: &Tag) -> Self {
        Entry {
            description: description.to_string(),
            vcs: *vcs,
            tag: *tag,
        }
    }
}

pub fn extract(
    vals: &HashMap<String, String>,
    builder: &mut apt_capnp::source::Builder,
) -> Result<()> {
    let mut found = Vec::with_capacity(4);

    for &(vcs_token, ref vcs) in
        [
            ("Arch", Vcs::Arch),
            ("Browser", Vcs::Browser),
            ("Browse", Vcs::Browser),
            ("Bzr", Vcs::Bzr),
            ("Cvs", Vcs::Cvs),
            ("Darcs", Vcs::Darcs),
            ("Git", Vcs::Git),
            ("Hg", Vcs::Hg),
            ("Mtn", Vcs::Mtn),
            ("Svn", Vcs::Svn),
        ].into_iter()
    {

        // Simplest form: Vcs-Git
        if let Some(x) = vals.get(&format!("Vcs-{}", vcs_token)) {
            found.push(Entry::new(x, vcs, &Tag::Vcs));
        }

        for &(tag_token, ref tag) in
            [
                ("Orig", Tag::Orig),
                ("Original", Tag::Orig),
                ("Debian", Tag::Debian),
                ("Upstream", Tag::Upstream),
            ].into_iter()
        {
            // Common form: Debian-Vcs-Git, Orig-Vcs-Browser, Original-Vcs-Bzr, Upstream-Vcs-Bzr
            if let Some(x) = vals.get(&format!("{}-Vcs-{}", tag_token, vcs_token)) {
                found.push(Entry::new(x, vcs, tag));
            }
            // Vcs-Upstream-Bzr seen in the wild
            else if let Some(x) = vals.get(&format!("Vcs-{}-{}", tag_token, vcs_token)) {
                found.push(Entry::new(x, vcs, tag));
            }
        }
    }

    let mut builder = builder.borrow().init_vcs(as_u32(found.len()));

    for (i, entry) in found.into_iter().enumerate() {
        let mut builder = builder.borrow().get(as_u32(i));
        builder.set_description(&entry.description);
        match entry.vcs {
            Vcs::Browser => builder.borrow().init_type().set_browser(()),
            Vcs::Arch => builder.borrow().init_type().set_arch(()),
            Vcs::Bzr => builder.borrow().init_type().set_bzr(()),
            Vcs::Cvs => builder.borrow().init_type().set_cvs(()),
            Vcs::Darcs => builder.borrow().init_type().set_darcs(()),
            Vcs::Git => builder.borrow().init_type().set_git(()),
            Vcs::Hg => builder.borrow().init_type().set_hg(()),
            Vcs::Mtn => builder.borrow().init_type().set_mtn(()),
            Vcs::Svn => builder.borrow().init_type().set_svn(()),
        }

        match entry.tag {
            Tag::Vcs => builder.borrow().init_tag().set_vcs(()),
            Tag::Orig => builder.borrow().init_tag().set_orig(()),
            Tag::Debian => builder.borrow().init_tag().set_debian(()),
            Tag::Upstream => builder.borrow().init_tag().set_upstream(()),
        }
    }

    Ok(())
}
