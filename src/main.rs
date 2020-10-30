use dirs_next as dirs;
use envsubst::{self, substitute};
use git_repo_manifest as manifest;
use git_repo_manifest::Manifest;
use gumdrop::Options;
use quick_error::quick_error;
use quick_xml as qxml;
use serde::ser::Serialize;

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::str::FromStr;
use std::{ffi, fs, io, path, str};

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
    TemplateNotFound(path: String) {
        display("template not found: {}", path)
    }
    }
}

#[derive(Debug, Options)]
struct Args {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "for all projects envsubst template", meta = "TEMPLATE")]
    projects: Option<String>,

    #[options(help = "for all remotes envsubst template", meta = "TEMPLATE")]
    remotes: Option<String>,

    #[options(
        short = 'm',
        long = "manifest",
        help = "(default) convert to a manifest to local_manifest with local.env",
        default = ".repo/manifest.xml"
    )]
    manifest: String,

    #[options(short = 'd', long = "dir", help = "template dir")]
    template_dir: Option<String>,
}

fn split_once(s: &str, delim: char) -> Option<(&str, &str)> {
    let pos = s.find(delim);
    pos.map(|idx| (&s[0..idx], &s[idx + delim.len_utf8()..]))
}

fn template_path(path: &Option<String>) -> Option<path::PathBuf> {
    if let Some(dir) = path {
        let mut pbuf = path::PathBuf::new();
        pbuf.push(dir);
        Some(pbuf)
    } else {
        dirs::config_dir()
    }
}

#[derive(Clone)]
enum TemplateKind {
    Project(String),
    Remote(String),
}

impl TemplateKind {
    fn path_setup(&self, p: &mut path::PathBuf) {
        let sub = match self {
            Self::Project(_) => "project",
            Self::Remote(_) => "remote",
        };
        p.extend(&["manifest-tool", sub]);
        p.push(self.to_string());
    }

    fn to_string(&self) -> &String {
        match self {
            Self::Project(s) | Self::Remote(s) => s,
        }
    }
}

impl Args {
    fn read_template_file(&self, k: TemplateKind) -> Result<String, crate::Error> {
        let s = k.to_string();
        let path = path::Path::new(&s);
        let mut template = String::new();
        if k.clone().to_string() == "-" {
            io::BufReader::new(io::stdin()).read_to_string(&mut template)?;
        } else if let Some(mut file_path) = template_path(&self.template_dir) {
            k.path_setup(&mut file_path);
            let mut f =
                fs::File::open(path).or(Err(Error::FileNotFound(Box::new(path.to_path_buf()))))?;
            f.read_to_string(&mut template)?;
        } else if path.exists() {
            let mut f =
                fs::File::open(path).or(Err(Error::FileNotFound(Box::new(path.to_path_buf()))))?;
            f.read_to_string(&mut template)?;
        } else {
            return Err(Error::TemplateNotFound(k.to_string().clone()));
        }

        Ok(template)
    }
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

fn main() -> Result<(), Error> {
    let args = Args::parse_args_default_or_exit();

    if let Some(template) = &args.projects {
        let template = args.read_template_file(TemplateKind::Project(template.clone()))?;
        let default_file = fs::File::open(path::Path::new(".repo/manifest.xml"))?;
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

    println!("Generating local manifest");
    if let Some(template) = &args.remotes {
        let template = args.read_template_file(TemplateKind::Remote(template.to_string()))?;
        let mut stdout = io::BufWriter::new(io::stdout());
        let mut context: HashMap<String, String> = HashMap::new();
        let default_file = fs::File::open(path::Path::new(".repo/manifest.xml"))?;
        let default_file = io::BufReader::new(default_file);
        let manifest: Manifest = manifest::de::from_reader(default_file)?;
        for remote in manifest.remotes() {
            remote.into_hash(&mut context);
            envsubst_write(&template, &mut stdout, &context)?;
        }
        return Ok(stdout.flush()?);
    }

    let config_file = dirs::config_dir().map(|mut dir| {
        dir.extend(&["manifest-tool", "local.env"]);
        dir
    });
    let mut config_str = String::new();

    if let Some(config_file) = config_file {
        let fd = fs::File::open(config_file.clone())
            .or(Err(Error::FileNotFound(Box::new(config_file))))?;
        let _ = io::BufReader::new(fd).read_to_string(&mut config_str)?;
    };

    if let Ok(dirs) = std::fs::read_dir(args.manifest) {
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
                let local_manifests_path = path::Path::new(".repo").join("local_manifests");
                fs::create_dir_all(local_manifests_path.clone())?;
                let local_manifest_path = local_manifests_path.join(file_name);
                let mut local_manifest_file = fs::File::create(local_manifest_path)?;
                let mut remotes = Vec::new();
                for remote in manifest.remotes() {
                    let name = remote.name();
                    let mut context = HashMap::new();
                    remote.into_hash(&mut context);
                    let config_subst = substitute(config_str.clone(), &context)?;
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
