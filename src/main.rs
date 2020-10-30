use dirs_next as dirs;
use git_repo_manifest as manifest;
use git_repo_manifest::Manifest;
use gumdrop::Options;
use quick_error::quick_error;

use envsubst;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::path;
use std::str;
use std::str::FromStr;

quick_error! {
    #[derive(Debug)]
    enum Error {
        Io(err: io::Error) {
            from()
        }
        Deserialization(err: manifest::de::DeError) {
            from()
        }
        Envsubst(err: envsubst::Error) {
            from()
        }

    ConfigFileFormat
    FetchRequired
    }
}

#[derive(Debug, Options)]
struct Args {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "specify a push url")]
    push_url: Option<String>,

    #[options(help = "specify a fetch url")]
    fetch_url: Option<String>,

    #[options(help = "specify a review url")]
    review_url: Option<String>,

    #[options(long = "override", help = "allow overriding duplicates")]
    override_dup: bool,

    #[options(help = "review protocol")]
    review_protocol: Option<git_repo_manifest::ReviewProtocolType>,

    #[options(free)]
    manifest_files: Vec<String>,
}

fn split_once<'a>(s: &'a str, delim: char) -> Option<(&'a str, &'a str)> {
    let pos = s.find(delim);
    pos.map(|idx| (&s[0..idx], &s[idx + delim.len_utf8()..]))
}

fn read_dot_env<T: io::Read>(fd: io::BufReader<T>) -> Result<HashMap<String, String>, Error> {
    let mut map = HashMap::new();

    for line in fd.lines() {
        if let Some((key, value)) = split_once(line?.as_str(), '=') {
            map.insert(key.to_string(), value.to_string());
        } else {
            return Err(Error::ConfigFileFormat);
        }
    }
    Ok(map)
}

fn main() -> Result<(), Error> {
    let args = Args::parse_args_default_or_exit();
    let config_file = dirs::config_dir().map(|mut dir| {
        dir.extend(&["manifest-tool", "config.env"]);
        dir
    });
    let mut config_str = String::new();

    if let Some(config_file) = config_file {
        let fd = fs::File::open(config_file)?;
        let _ = io::BufReader::new(fd).read_to_string(&mut config_str)?;
    };

    // FIXME this branch is pretty terrible, we aren't doing anything if args *are* given,
    // and should refactor the contents into some other function..
    // that said this is just a quick hack at an ad-hoc utility so it works for now.
    if args.manifest_files.len() == 0 {
        if let Ok(dirs) = std::fs::read_dir(".repo/manifests") {
            for dir_entry in dirs {
                let dir_entry = dir_entry?;
                let file_name = dir_entry.file_name();
                let extension = path::Path::new(&file_name)
                    .extension()
                    .and_then(OsStr::to_str);
                if extension == Some("xml") {
                    let file = io::BufReader::new(fs::File::open(dir_entry.path())?);
                    let manifest: Manifest = manifest::de::from_reader(file)?;
                    let local_manifests_path = path::Path::new(".repo").join("local_manifests");
                    fs::create_dir_all(local_manifests_path.clone())?;
                    let local_manifest_path = local_manifests_path.join(file_name);
                    let mut local_manifest_file = fs::File::create(local_manifest_path)?;
                    let mut remotes = Vec::new();
                    for remote in manifest.remotes() {
                        let name = remote.name();
                        let to_subst = vec![("remote_name".to_string(), name.to_string())];
                        let context: HashMap<_, _> = to_subst.into_iter().collect();
                        let config_subst = envsubst::substitute(config_str.clone(), &context)?;
                        let mut config = read_dot_env(io::BufReader::new(config_subst.as_bytes()))?;
                        let mut args_map: HashMap<String, String> = HashMap::new();
                        if let Some(push_url) = args.push_url.clone() {
                            args_map.insert(
                                "push_url".to_string(),
                                envsubst::substitute(push_url, &context)?,
                            );
                        }
                        if let Some(fetch_url) = args.fetch_url.clone() {
                            args_map.insert(
                                "fetch_url".to_string(),
                                envsubst::substitute(fetch_url, &context)?,
                            );
                        }
                        if let Some(review_url) = args.review_url.clone() {
                            args_map.insert(
                                "review_url".to_string(),
                                envsubst::substitute(review_url, &context)?,
                            );
                        }

                        if let Some(review_protocol) = args.review_url.clone() {
                            args_map.insert(
                                "review_protocol".to_string(),
                                String::from(review_protocol),
                            );
                        }

                        config.extend(args_map);
                        if let Some(fetch_url) = config.get("fetch_url") {
                            let local_remote = manifest::Remote::new(
                                name.clone(),
                                None,
                                config.get("push_url").map(|s| s.clone()),
                                fetch_url.to_string(),
                                config.get("review_url").map(|s| s.clone()),
                                None,
                                config
                                    .get("review_protocol")
                                    .map(|s| manifest::ReviewProtocolType::from_str(s).unwrap()),
                                Some(true),
                            );
                            remotes.push(local_remote);
                        } else {
                            return Err(Error::FetchRequired);
                        }
                    }
                    let manifest: Manifest = Manifest::new(
                        None,
                        None,
                        remotes,
                        None,
                        vec![],
                        vec![],
                        vec![],
                        None,
                        vec![],
                    );
                    manifest::se::to_writer(&mut local_manifest_file, &manifest)?
                }
            }
        }
    }
    Ok(())
}
