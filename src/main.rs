use toml::to_string_pretty;
mod lib;
use lib::CrateInfo;
use std::collections::HashMap;
use std::path::PathBuf;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref BASE_PATH: PathBuf = {
        let base = if cfg!(windows) {
            dirs::data_dir().expect("Unable to access data dir on windows")
        } else {
            dirs::home_dir().expect("Unable to access home dir")
        };
        base.join("crate_change_monitor")
    };
}

static DIV: &str = "----------";

fn main() -> Result<(), Box<dyn ::std::error::Error>> {
    println!("working from {}", BASE_PATH.display());
    let known = get_known()?;
    let server = get_server()?;
    let changes = lib::perform_analysis(&server, &known);
    let server_map: HashMap<String, &CrateInfo> = server.iter().map(|c| (c.name.to_string(), c)).collect();
    let mut send = false;
    let mut message = String::from("Crate Change Report");
    message.push('\n');
    message.push_str(DIV);
    message.push('\n');
    message.push_str(DIV);
    message.push('\n');
    for (name, change) in changes {
        if change.any_changes() {
            send = true;
            message.push_str(DIV);
            if let Some(info) = server_map.get(&name) {
                message.push_str(&to_string_pretty(&info)?);
                message.push('\n');
            } else {
                message.push_str(&format!("unknown changes for {}\n", name))
            }
        }
    }
    for ref c in server {
        write_crate_info(c)?;
    }
    if send {
        lib::send_message(&message)?;
    }
    println!("COMPLETE!");
    Ok(())
}

fn write_crate_info(c: &CrateInfo) -> Result<(), Box<dyn ::std::error::Error>> {
    let path = BASE_PATH.join(&c.name);
    ::std::fs::write(&path, to_string_pretty(c)?)?;
    Ok(())
}

fn get_server() -> Result<Vec<CrateInfo>, Box<dyn ::std::error::Error>> {
    println!("getting user's crates");
    let crates = act_and_wait(1000, || lib::get_crates(lib::OWNER))?;
    crates.into_iter().map(|(mut c, url)| {
        println!("getting rev deps for {}", c.name);
        c.reverse_deps = act_and_wait(1000, || lib::get_rev_deps(&url))?.versions;
        Ok(c)
    }).collect()
}

fn get_known() -> Result<Vec<CrateInfo>, Box<dyn ::std::error::Error>> {
    let mut ret = Vec::new();
    if !BASE_PATH.exists() {
        ::std::fs::create_dir_all(BASE_PATH.as_path())?;
    }
    for file in ::std::fs::read_dir(BASE_PATH.as_path())? {
        if let Ok(file) = file {
            let path = file.path();
            if let Some(ext) = path.extension() {
                if ext == "toml" {
                    let s = ::std::fs::read_to_string(path)?;
                    ret.push(toml::from_str(&s)?);
                }
            }
        }
    }
    Ok(ret)
}

fn act_and_wait<T, F>(length: u64, action: F) -> T 
where F: Fn() -> T {
    let ret = action();
    ::std::thread::sleep(::std::time::Duration::from_millis(length));
    ret
}
