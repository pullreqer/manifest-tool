use clap;
use dirs_next as dirs;
use envsubst::{self, substitute};
use git_repo_manifest as manifest;
use git_repo_manifest::Manifest;
use quick_error::quick_error;
use quick_xml as qxml;
use serde::ser::Serialize;

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::str::FromStr;
use std::{env, ffi, fs, io, path, str};

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

        FileNotFound(p: Box<path::PathBuf>) {
            display("file not found: {:#?}\n", p)
        }
        ConfigFileFormat
        FetchRequired
        UnspecifiedQuantifier
        UnknownConfigPath
        TemplateNotFound(path: String) {
            display("template not found: {}", path)
        }
    }
}

fn split_once(s: &str, delim: char) -> Option<(&str, &str)> {
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

fn envsubst_write(
    template_string: &'_ str,
    output: &mut dyn io::Write,
    contents: &HashMap<String, String>,
) -> Result<(), Error> {
    let s = substitute(template_string, &contents)?;
    Ok(output.write_all(s.as_bytes())?)
}

trait IntoHash<K, V> {
    fn into_hash(&self, context: &mut HashMap<K, V>);
}

impl IntoHash<String, String> for manifest::Remote {
    fn into_hash<'a>(&self, context: &'a mut HashMap<String, String>) {
        context.insert("remote_name".to_string(), self.name().to_string());
        let () = self
            .pushurl()
            .iter()
            .map(|pushurl| {
                let _ = context.insert("push_url".to_string(), pushurl.to_string());
            })
            .collect();
        context.insert("fetch_url".to_string(), self.fetch().to_string());
        let () = self
            .review()
            .iter()
            .map(|review| {
                let _ = context.insert("review_url".to_string(), review.to_string());
            })
            .collect();
    }
}

struct ManifestArg {
    template: path::PathBuf,
    manifest_dir: path::PathBuf,
    local_manifest_dir: path::PathBuf,
}

struct EnvArg {
    template: path::PathBuf,
    manifest: path::PathBuf,
}

enum Mode {
    Projects(EnvArg),
    Remotes(EnvArg),
    Convert(ManifestArg),
}

fn args<'a>(config_dir: path::PathBuf) -> Result<clap::ArgMatches<'a>, clap::Error> {
    use clap::Arg;
    let app_name = "manifest-tool";
    let config_dir = config_dir.join(app_name);
    let convert_dir = config_dir.join("convert").into_os_string();
    let remote_dir = config_dir.join("remotes").into_os_string();
    let project_dir = config_dir.join("projects").into_os_string();

    let app = clap::App::new(app_name)
        .arg(
            Arg::with_name("convert")
                .short("c")
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name("remotes")
                .short("r")
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name("projects")
                .short("p")
                .takes_value(false)
                .required(false),
        )
        .group(
            clap::ArgGroup::with_name("mode")
                .args(&["convert", "projects", "remotes"])
                .required(true),
        )
        .arg(
            Arg::with_name("template-file")
                .short("t")
                .takes_value(true)
                .required(false)
                .default_value("default.env"),
        )
        .arg(
            Arg::with_name("template-dir")
                .short("T")
                .takes_value(true)
                .required(false)
                .default_value_ifs_os(&[
                    ("convert", None, &convert_dir),
                    ("remotes", None, &remote_dir),
                    ("projects", None, &project_dir),
                ]),
        )
        .arg(
            Arg::with_name("manifest-dir")
                .short("D")
                .takes_value(true)
                .required(false)
                .default_value(".repo/manifests"),
        )
        .arg(
            Arg::with_name("manifest-dest")
                .short("M")
                .takes_value(true)
                .default_value_if("convert", None, ".repo/local_manifests")
                .required(false),
        )
        .arg(
            Arg::with_name("manifest-file")
                .short("m")
                .takes_value(true)
                .default_value("../manifest.xml") // ¯\_(ツ)_/¯
                .required(false),
        )
        .arg(
            Arg::with_name("TEMPLATE")
                .takes_value(true)
                .required(false)
                .default_value_ifs(&[
                    ("convert", None, "unused"),
                    ("remotes", None, "default.env"),
                    ("projects", None, "default.env"),
                ]),
        );
    app.get_matches_from_safe(env::args())
}

fn mode_for_args<'a>(config_dir: path::PathBuf) -> Mode {
    match args(config_dir) {
        Err(err) => err.exit(),
        Ok(arg) => {
            let template = path::PathBuf::from(arg.value_of("template-dir").unwrap())
                .join(arg.value_of("template-file").unwrap());

            if arg.is_present("convert") {
                Mode::Convert(ManifestArg {
                    template,
                    manifest_dir: path::PathBuf::from(arg.value_of("manifest-dir").unwrap()),
                    local_manifest_dir: path::PathBuf::from(arg.value_of("manifest-dest").unwrap()),
                })
            } else {
                let manifest = path::PathBuf::from(arg.value_of("manifest-dir").unwrap())
                    .join(arg.value_of("manifest-file").unwrap());
                let env_arg = EnvArg { template, manifest };
                if arg.is_present("remotes") {
                    Mode::Remotes(env_arg)
                } else {
                    Mode::Projects(env_arg)
                }
            }
        }
    }
}

fn projects_cmd(arg: EnvArg) -> Result<(), Error> {
    let mut template = String::new();
    fs::File::open(arg.template)?.read_to_string(&mut template)?;
    let default_file = fs::File::open(arg.manifest)?;
    let default_file = io::BufReader::new(default_file);
    let mut manifest: Manifest = manifest::de::from_reader(default_file)?;
    manifest.set_defaults();
    let mut remote_hash = HashMap::new();
    manifest.remotes().iter().for_each(|remote| {
        remote_hash.insert(remote.name().to_string(), remote);
    });

    let mut stdout = io::BufWriter::new(io::stdout());
    for project in manifest.projects() {
        let mut context: HashMap<String, String> = HashMap::new();
        if let Some(remote_name) = project.remote() {
            if let Some(remote) = remote_hash.get(remote_name) {
                remote.into_hash(&mut context);
            }
        }
        context.insert("project_name".to_string(), project.name().to_string());
        envsubst_write(&template, &mut stdout, &context)?;
    }
    return Ok(stdout.flush()?);
}

fn remotes_cmd(arg: EnvArg) -> Result<(), Error> {
    let mut template = String::new();
    fs::File::open(arg.template)?.read_to_string(&mut template)?;
    let mut stdout = io::BufWriter::new(io::stdout());
    let mut context: HashMap<String, String> = HashMap::new();
    let manifest_file = fs::File::open(arg.manifest)?;
    let manifest_file = io::BufReader::new(manifest_file);
    let manifest: Manifest = manifest::de::from_reader(manifest_file)?;
    for remote in manifest.remotes() {
        remote.into_hash(&mut context);
        envsubst_write(&template, &mut stdout, &context)?;
    }
    return Ok(stdout.flush()?);
}

fn convert_cmd(arg: ManifestArg) -> Result<(), Error> {
    let mut template = String::new();
    fs::File::open(arg.template)?.read_to_string(&mut template)?;
    if let Ok(dirs) = std::fs::read_dir(arg.manifest_dir) {
        for dir_entry in dirs {
            let dir_entry = dir_entry?;
            let file_name = dir_entry.file_name();
            let extension = path::Path::new(&file_name)
                .extension()
                .and_then(ffi::OsStr::to_str);
            if extension == Some("xml") {
                let file = io::BufReader::new(
                    fs::File::open(dir_entry.path())
                        .or(Err(Error::FileNotFound(Box::new(dir_entry.path()))))?,
                );
                let manifest: Manifest = manifest::de::from_reader(file)?;
                fs::create_dir_all(arg.local_manifest_dir.clone())?;
                let local_manifest_path = arg.local_manifest_dir.clone().join(file_name);
                let mut local_manifest_file = fs::File::create(local_manifest_path)?;
                let mut remotes = Vec::new();
                for remote in manifest.remotes() {
                    let name = remote.name();
                    let mut context = HashMap::new();
                    remote.into_hash(&mut context);
                    let config_subst = substitute(template.clone(), &context)?;
                    let config = read_dot_env(io::BufReader::new(config_subst.as_bytes()))?;

                    if let Some(fetch_url) = config.get("fetch_url") {
                        let local_remote = manifest::Remote::new(
                            name.clone(),
                            None,
                            config.get("push_url").cloned(),
                            fetch_url.to_string(),
                            config.get("review_url").cloned(),
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
                let writer = qxml::Writer::new_with_indent(&mut local_manifest_file, b'\t', 1);
                let mut ser = manifest::se::Serializer::with_root(writer, None);
                manifest.serialize(&mut ser)?;
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    if let Some(config_dir) = dirs::config_dir() {
        match mode_for_args(config_dir) {
            Mode::Projects(arg) => projects_cmd(arg),

            Mode::Remotes(arg) => remotes_cmd(arg),

            Mode::Convert(arg) => convert_cmd(arg),
        }
    } else {
        Err(Error::UnknownConfigPath)
    }
}
