use std::collections::HashSet;

use reqwest::Url;

use errors::*;

#[derive(PartialOrd, Ord, Hash, PartialEq, Eq)]
pub struct RequestedRelease {
    mirror: Url,
    /// This can also be called "suite" in some places,
    /// e.g. "unstable" (suite) == "sid" (codename)
    codename: String,
}

impl RequestedRelease {
    pub fn dists(&self) -> Result<Url> {
        Ok(self.mirror.join("dists/")?.join(
            &format!("{}/", self.codename),
        )?)
    }
}

fn releases(sources_list: &[::classic_sources_list::Entry]) -> Result<Vec<RequestedRelease>> {
    let mut ret = HashSet::with_capacity(sources_list.len() / 2);

    for entry in sources_list {
        ret.insert(RequestedRelease {
            // TODO: urls without trailing slashes?
            mirror: Url::parse(&entry.url)?,
            codename: entry.suite_codename.to_string(),
        });
    }

    Ok(ret.into_iter().collect())
}
