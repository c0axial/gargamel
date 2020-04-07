extern crate rpassword;

use std::io;
use simplelog::{CombinedLogger, TermLogger, WriteLogger, Config, TerminalMode, LevelFilter};
use std::fs::{File, create_dir_all};
use crate::logo::print_logo;
use crate::arg_parser::Opts;

#[macro_use]
extern crate log;
extern crate simplelog;

use clap::derive::Clap;
use crate::evidence_acquirer::EvidenceAcquirer;
use std::path::{Path, PathBuf};
use crate::remote::{Computer, Copier, XCopy, PsCopy, WindowsRemoteCopier, Scp, RdpCopy};
use crate::memory_acquirer::MemoryAcquirer;
use crate::command_runner::CommandRunner;
use crate::file_acquirer::download_files;
use rpassword::read_password;
use crate::registry_acquirer::RegistryAcquirer;
use std::time::Duration;

mod process_runner;
mod evidence_acquirer;
mod remote;
mod arg_parser;
mod logo;
mod memory_acquirer;
mod command_utils;
mod utils;
mod file_acquirer;
mod registry_acquirer;
mod command_runner;

fn setup_logger() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Trace, Config::default(), TerminalMode::Mixed).unwrap(),
            WriteLogger::new(LevelFilter::Trace, Config::default(), File::create("gargamel.log").unwrap()),
        ]
    ).unwrap();
}

fn main() -> Result<(), io::Error> {
    setup_logger();
    print_logo();

    let opts: Opts = Opts::parse();
    create_dir_all(&opts.store_directory)?;

    let opts = match &opts.password {
        Some(_) => opts,
        None => {
            println!("Password: ");
            let password = read_password().expect("Error reading password");
            Opts { password: Some(password), ..opts }
        }
    };

    let remote_computer = Computer::from(opts.clone());
    let local_store_directory = Path::new(&opts.store_directory);
    let key_file = opts.ssh_key.clone().map(|it| PathBuf::from(it));
    let evidence_acquirers = create_evidence_acquirers(
        &remote_computer,
        local_store_directory,
        &opts,
        key_file.as_ref().map(|it| it.to_path_buf()),
    );
    for acquirer in evidence_acquirers {
        acquirer.run_all();
    }
    let registry_acquirers = create_registry_acquirers(
        &remote_computer,
        local_store_directory,
        &opts,
    );
    for acquirer in registry_acquirers {
        acquirer.acquire();
    }
    if opts.custom_command_path.is_some() {
        let command_runners = create_command_runners(
            &remote_computer,
            local_store_directory,
            &opts,
            key_file.as_ref().map(|it| it.to_path_buf()),
        );
        for command_runner in command_runners {
            info!("Running commands using method {}", command_runner.connector.connect_method_name());
            command_runner.run_commands(Path::new(opts.custom_command_path.as_ref().unwrap()));
        }
    }
    if opts.search_files_path.is_some() {
        let search_files_path = opts.search_files_path.as_ref().unwrap();
        let search_files_path = Path::new(search_files_path);
        if opts.ssh {
            let remote_copier = Scp {
                computer: remote_computer.clone(),
                key_file: key_file.as_ref().map(|it| it.clone()),
            };
            download_files(
                search_files_path,
                local_store_directory,
                &remote_copier,
            )?;
        } else {
            let copiers = create_windows_non_rdp_file_copiers(&opts);
            let mut non_rdp_success = false;
            for copier in copiers.into_iter() {
                info!("Downloading specified files using {}",  copier.method_name());
                let remote_copier = WindowsRemoteCopier::new(
                    remote_computer.clone(),
                    copier,
                );
                let result = download_files(
                    search_files_path,
                    local_store_directory,
                    &remote_copier,
                );
                if result.is_ok() {
                    info!("Files in {} successfully transferred.", search_files_path.display());
                    non_rdp_success = true;
                    break;
                }
            }
            if !non_rdp_success && (opts.rdp || opts.all) {
                let remote_copier = RdpCopy{
                    computer: remote_computer.clone(),
                    nla: opts.nla
                };
                info!("Downloading specified files using {}",  remote_copier.method_name());
                let result = download_files(
                    search_files_path,
                    local_store_directory,
                    &remote_copier,
                );
                if result.is_ok() {
                    info!("Files in {} successfully transferred.", search_files_path.display());
                }
            }

        }
    }
    if opts.image_memory.is_some() {
        let memory_acquirers = create_memory_acquirers(
            &remote_computer,
            local_store_directory,
            &opts,
        );
        let image_memory_remote_store = opts.image_memory.as_ref().unwrap();
        for acquirer in memory_acquirers {
            info!("Running memory acquirer using method {}", acquirer.connector.connect_method_name());
            let image_res = acquirer.image_memory(Path::new(image_memory_remote_store.as_str()));
            if image_res.is_ok() {
                break;
            }
        }

    }

    Ok(())
}

fn create_evidence_acquirers<'a>(
    computer: &'a Computer,
    local_store_directory: &'a Path,
    opts: &Opts,
    key_file: Option<PathBuf>,
) -> Vec<EvidenceAcquirer<'a>> {
    let acquirers: Vec<EvidenceAcquirer<'a>> = if opts.all {
        vec![
            EvidenceAcquirer::psexec(
                computer,
                local_store_directory,
            ),
            EvidenceAcquirer::wmi(
                computer,
                local_store_directory,
            ),
            EvidenceAcquirer::psremote(
                computer,
                local_store_directory,
            ),
            EvidenceAcquirer::rdp(
                computer,
                local_store_directory,
                opts.nla
            ),
        ]
    } else {
        let mut acquirers = Vec::<EvidenceAcquirer<'a>>::new();
        if opts.psexec {
            acquirers.push(
                EvidenceAcquirer::psexec(
                    computer,
                    local_store_directory,
                ),
            );
        }
        if opts.wmi {
            acquirers.push(
                EvidenceAcquirer::wmi(
                    computer,
                    local_store_directory,
                ),
            );
        }
        if opts.psrem {
            acquirers.push(
                EvidenceAcquirer::psremote(
                    computer,
                    local_store_directory,
                )
            );
        }
        if opts.local {
            acquirers.push(
                EvidenceAcquirer::local(
                    computer,
                    local_store_directory,
                )
            )
        }
        if opts.ssh {
            acquirers.push(
                EvidenceAcquirer::ssh(
                    computer,
                    local_store_directory,
                    key_file,
                )
            )
        }
        if opts.rdp {
            acquirers.push(
                EvidenceAcquirer::rdp(
                    computer,
                    local_store_directory,
                    opts.nla
                ),
            )
        }
        acquirers
    };
    acquirers
}

fn create_memory_acquirers<'a>(
    computer: &'a Computer,
    local_store_directory: &'a Path,
    opts: &Opts,
) -> Vec<MemoryAcquirer<'a>> {
    let acquirers: Vec<MemoryAcquirer<'a>> = if opts.all {
        vec![
            MemoryAcquirer::psexec(
                computer,
                local_store_directory,
            ),
            MemoryAcquirer::psremote(
                computer,
                local_store_directory,
            ),
            MemoryAcquirer::rdp(
                computer,
                local_store_directory,
                Duration::from_secs(60 * opts.rdp_wait_time),
                opts.nla
            ),
        ]
    } else {
        let mut acquirers = Vec::<MemoryAcquirer>::new();
        if opts.psexec {
            acquirers.push(
                MemoryAcquirer::psexec(
                    computer,
                    local_store_directory,
                )
            );
        }
        if opts.psrem {
            acquirers.push(
                MemoryAcquirer::psremote(
                    computer,
                    local_store_directory,
                )
            );
        }
        if opts.rdp {
            acquirers.push(
                MemoryAcquirer::rdp(
                    computer,
                    local_store_directory,
                    Duration::from_secs(60 * opts.rdp_wait_time),
                    opts.nla
                )
            );
        }
        acquirers
    };
    acquirers
}

fn create_command_runners<'a>(
    computer: &'a Computer,
    local_store_directory: &'a Path,
    opts: &Opts,
    key_file: Option<PathBuf>,
) -> Vec<CommandRunner<'a>> {
    let acquirers: Vec<CommandRunner<'a>> = if opts.all {
        vec![
            CommandRunner::psexec(
                computer,
                local_store_directory,
            ),
            CommandRunner::psremote(
                computer,
                local_store_directory,
            ),
        ]
    } else {
        let mut acquirers = Vec::<CommandRunner>::new();
        if opts.psexec {
            acquirers.push(
                CommandRunner::psexec(
                    computer,
                    local_store_directory,
                )
            );
        }
        if opts.psrem {
            acquirers.push(
                CommandRunner::psremote(
                    computer,
                    local_store_directory,
                )
            );
        }
        if opts.local {
            acquirers.push(
                CommandRunner::local(
                    computer,
                    local_store_directory,
                )
            )
        }
        if opts.ssh {
            acquirers.push(
                CommandRunner::ssh(
                    computer,
                    local_store_directory,
                    key_file,
                )
            )
        }
        if opts.wmi {
            acquirers.push(
                CommandRunner::wmi(
                    computer,
                    local_store_directory,
                )
            )
        }
        if opts.rdp {
            acquirers.push(
                CommandRunner::rdp(
                    computer,
                    local_store_directory,
                    opts.nla
                )
            )
        }
        acquirers
    };
    acquirers
}

fn create_registry_acquirers<'a>(
    computer: &'a Computer,
    local_store_directory: &'a Path,
    opts: &Opts,
) -> Vec<RegistryAcquirer<'a>> {
    let acquirers: Vec<RegistryAcquirer<'a>> = if opts.all {
        vec![
            RegistryAcquirer::psexec(
                computer,
                local_store_directory,
            ),
            RegistryAcquirer::psremote(
                computer,
                local_store_directory,
            ),
            RegistryAcquirer::rdp(
                computer,
                local_store_directory,
                opts.nla
            ),
        ]
    } else {
        let mut acquirers = Vec::<RegistryAcquirer<'a>>::new();
        if opts.psexec {
            acquirers.push(
                RegistryAcquirer::psexec(
                    computer,
                    local_store_directory,
                ),
            );
        }
        if opts.psrem {
            acquirers.push(
                RegistryAcquirer::psremote(
                    computer,
                    local_store_directory,
                )
            );
        }
        if opts.rdp {
            acquirers.push(
                RegistryAcquirer::rdp(
                    computer,
                    local_store_directory,
                    opts.nla
                ),
            )
        }
        acquirers
    };
    acquirers
}

fn create_windows_non_rdp_file_copiers(opts: &Opts) -> Vec<Box<dyn Copier>> {
    let acquirers: Vec<Box<dyn Copier>> = if opts.all {
        vec![
            Box::new(XCopy {}),
            Box::new(PsCopy {})
        ]
    } else {
        let mut acquirers = Vec::<Box<dyn Copier>>::new();
        if opts.psexec {
            acquirers.push(
                Box::new(XCopy {})
            )
        }
        if opts.psrem {
            acquirers.push(
                Box::new(PsCopy {})
            );
        }
        acquirers
    };
    acquirers
}

