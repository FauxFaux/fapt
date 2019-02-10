use failure::Error;

use super::rfc822;
use super::types::RfcMapExt;
use super::types::Vcs;
use super::types::VcsTag;
use super::types::VcsType;

pub fn extract(map: &mut rfc822::Map) -> Result<Vec<Vcs>, Error> {
    let mut found = Vec::with_capacity(4);

    for (vcs_token, vcs) in &[
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
    ] {
        // Simplest form: Vcs-Git
        if let Some(description) = map.remove_one_line(&format!("Vcs-{}", vcs_token))? {
            found.push(Vcs {
                description: description.to_string(),
                vcs: *vcs,
                tag: VcsTag::Vcs,
            });
        }

        for (tag_token, tag) in &[
            ("Orig", VcsTag::Orig),
            ("Original", VcsTag::Orig),
            ("Debian", VcsTag::Debian),
            ("Upstream", VcsTag::Upstream),
        ] {
            // Common form: Debian-Vcs-Git, Orig-Vcs-Browser, Original-Vcs-Bzr, Upstream-Vcs-Bzr
            if let Some(description) =
                map.remove_one_line(format!("{}-Vcs-{}", tag_token, vcs_token))?
            {
                found.push(Vcs {
                    description: description.to_string(),
                    vcs: *vcs,
                    tag: *tag,
                });
            }
            // Vcs-Upstream-Bzr seen in the wild
            else if let Some(description) =
                map.remove_one_line(format!("Vcs-{}-{}", tag_token, vcs_token))?
            {
                found.push(Vcs {
                    description: description.to_string(),
                    vcs: *vcs,
                    tag: *tag,
                });
            }
        }
    }

    Ok(found)
}
