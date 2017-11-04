use std::collections::HashMap;

use apt_capnp;
use as_u32;
use errors::*;

use apt_capnp::VcsTag;
use apt_capnp::VcsType;

#[derive(Clone)]
struct Entry {
    description: String,
    vcs: VcsType,
    tag: VcsTag,
}

impl Entry {
    pub fn new(description: &str, vcs: &VcsType, tag: &VcsTag) -> Self {
        Entry {
            description: description.to_string(),
            vcs: *vcs,
            tag: *tag,
        }
    }
}

pub fn extract(vals: &HashMap<&str, &str>, builder: &mut apt_capnp::source::Builder) -> Result<()> {
    let mut found = Vec::with_capacity(4);

    for &(vcs_token, ref vcs) in
        [
            ("Arch", VcsType::Arch),
            ("Browser", VcsType::Browser),
            ("Browse", VcsType::Browser),
            ("Bzr", VcsType::Bzr),
            ("Cvs", VcsType::Cvs),
            ("Darcs", VcsType::Darcs),
            ("Git", VcsType::Git),
            ("Hg", VcsType::Hg),
            ("Mtn", VcsType::Mtn),
            ("Svn", VcsType::Svn),
        ].into_iter()
    {

        // Simplest form: Vcs-Git
        if let Some(x) = vals.get(format!("Vcs-{}", vcs_token).as_str()) {
            found.push(Entry::new(x, vcs, &VcsTag::Vcs));
        }

        for &(tag_token, ref tag) in
            [
                ("Orig", VcsTag::Orig),
                ("Original", VcsTag::Orig),
                ("Debian", VcsTag::Debian),
                ("Upstream", VcsTag::Upstream),
            ].into_iter()
        {
            // Common form: Debian-Vcs-Git, Orig-Vcs-Browser, Original-Vcs-Bzr, Upstream-Vcs-Bzr
            if let Some(x) = vals.get(format!("{}-Vcs-{}", tag_token, vcs_token).as_str()) {
                found.push(Entry::new(x, vcs, tag));
            }
            // Vcs-Upstream-Bzr seen in the wild
            else if let Some(x) = vals.get(format!("Vcs-{}-{}", tag_token, vcs_token).as_str()) {
                found.push(Entry::new(x, vcs, tag));
            }
        }
    }

    let mut builder = builder.borrow().init_vcs(as_u32(found.len()));

    for (i, entry) in found.into_iter().enumerate() {
        let mut builder = builder.borrow().get(as_u32(i));
        builder.set_description(&entry.description);
        builder.set_type(entry.vcs);
        builder.set_tag(entry.tag);
    }

    Ok(())
}
