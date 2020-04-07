use crate::remote::{Connector, Computer, Copier, XCopy, RemoteCopier};
use std::path::{Path, PathBuf};
use std::io;

pub struct Local {
    localhost: Computer
}

impl Local {
    pub fn new() -> Local {
        Local {
            localhost: Computer {
                address: String::from("127.0.0.1"),
                username: String::new(),
                password: None,
                domain: None,
            }
        }
    }
}

impl Connector for Local {
    fn connect_method_name(&self) -> &'static str {
        return "LOCAL";
    }

    fn prepare_command(&self,
                       _remote_computer: &Computer,
                       command: Vec<String>,
                       _output_file_path: Option<String>,
                       _elevated: bool,
    ) -> Vec<String> {
        command
    }
}

impl Copier for Local {
    fn copy_file(&self, source: &Path, target: &Path) -> io::Result<()> {
        let xcopy = XCopy {};
        xcopy.copy_file(source, target)
    }

    fn delete_file(&self, target: &Path) -> io::Result<()> {
        let xcopy = XCopy {};
        xcopy.delete_file(target)
    }

    fn method_name(&self) -> &'static str {
        "XCopy (local)"
    }
}

impl RemoteCopier for Local {
    fn computer(&self) -> &Computer {
        &self.localhost
    }

    fn copier_impl(&self) -> &dyn Copier {
        self
    }

    fn path_to_remote_form(&self, path: &Path) -> PathBuf {
        path.to_path_buf()
    }
}