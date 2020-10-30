use derive_getters::Getters;
use derive_new::new;
use quick_error::quick_error;
pub use quick_xml::de;
pub use quick_xml::se;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::str::FromStr;
#[cfg(test)]
mod test;

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
#[serde(rename = "manifest")]
pub struct Manifest {
    notice: Option<Notice>,
    manifest_server: Option<ManifestServer>,

    #[serde(rename = "remote", default)]
    remotes: Vec<Remote>,

    default: Option<DefaultTag>,

    #[serde(rename = "remove-project", default)]
    remove_projects: Vec<RemoveProject>,

    #[serde(rename = "project", default)]
    projects: Vec<Project>,

    #[serde(rename = "extend-project", default)]
    extend_projects: Vec<ExtendProject>,

    #[serde(rename = "repo-hooks")]
    repo_hooks: Option<RepoHooks>,

    #[serde(rename = "include", default)]
    includes: Vec<Include>,
}

impl Manifest {
    // FIXME add more defaults.
    pub fn set_defaults(&mut self) {
        if let Some(default) = &self.default {
            if let Some(remote) = &default.remote {
                for project in &mut self.projects {
                    if project.remote == None {
                        project.remote = Some(remote.to_string());
                    }
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct Notice {
    #[serde(rename = "$value", default)]
    notice: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct Remote {
    name: String,
    alias: Option<String>,
    pushurl: Option<String>,
    fetch: String,
    review: Option<String>,
    revision: Option<String>,
    // https://git-repo.info extensions
    r#type: Option<ReviewProtocolType>,
    r#override: Option<bool>,
}

quick_error! {
    #[derive(Debug)]
    pub enum ProtocolTypeError {
        UnexpectedProtocol(source: String) {
            display("Unknown protocol: {}", source)
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Copy, Clone)]
#[serde(try_from = "String")]
pub enum ReviewProtocolType {
    AGit,
    Gerrit,
}

impl TryFrom<String> for ReviewProtocolType {
    type Error = ProtocolTypeError;
    fn try_from(it: String) -> Result<Self, ProtocolTypeError> {
        use ReviewProtocolType as T;
        let ok = |proto| Ok(proto);
        match it.to_lowercase() {
            x if x == "agit" => ok(T::AGit),
            x if x == "gerrit" => ok(T::Gerrit),
            _ => Err(ProtocolTypeError::UnexpectedProtocol(it)),
        }
    }
}

impl FromStr for ReviewProtocolType {
    type Err = ProtocolTypeError;

    fn from_str(it: &str) -> Result<Self, Self::Err> {
        use ReviewProtocolType as T;
        let ok = |proto| Ok(proto);
        match it.to_lowercase() {
            x if x == "agit" => ok(T::AGit),
            x if x == "gerrit" => ok(T::Gerrit),
            _ => Err(ProtocolTypeError::UnexpectedProtocol(it.to_string())),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct DefaultTag {
    remote: Option<String>,
    revision: Option<String>,

    #[serde(rename = "dest-branch")]
    dest_branch: Option<String>,

    upstream: Option<String>,

    #[serde(rename = "sync-j")]
    sync_j: Option<String>,

    #[serde(rename = "sync-c")]
    sync_c: Option<String>,

    #[serde(rename = "sync-s")]
    sync_s: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct ManifestServer {
    url: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct RemoveProject {
    name: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct ExtendProject {
    name: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct Project {
    name: String,
    path: Option<String>,
    remote: Option<String>,
    revision: Option<String>,

    #[serde(rename = "dest-branch")]
    dest_branch: Option<String>,

    groups: Option<String>,
    rebase: Option<String>,

    #[serde(rename = "sync-c")]
    sync_c: Option<String>,

    #[serde(rename = "sync-s")]
    sync_s: Option<String>,

    #[serde(rename = "sync-tags")]
    sync_tags: Option<String>,

    upstream: Option<String>,

    #[serde(rename = "clone-depth")]
    clone_depth: Option<usize>,

    #[serde(rename = "force-path")]
    force_path: Option<String>,
}

fn deserialize_space_separated<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    // Not sure if enabled-list should allow empty strings, this does currently.
    Ok(buf.split_whitespace().map(|s| s.to_string()).collect())
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct RepoHooks {
    #[serde(rename = "in-project")]
    in_project: String,
    #[serde(
        rename = "enabled-list",
        deserialize_with = "deserialize_space_separated"
    )]
    enabled_list: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Getters, new)]
pub struct Include {
    name: String,
}
