use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub enum BinaryKind {
    Main,
    Runner,
}

impl BinaryKind {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Main => "previa-main",
            Self::Runner => "previa-runner",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Runner => "runner",
        }
    }

    pub fn all() -> [Self; 2] {
        [Self::Main, Self::Runner]
    }
}

#[derive(Clone, Debug)]
pub struct InstalledVersion {
    pub tag: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct LinuxArch {
    pub slug: &'static str,
    pub alt: &'static str,
    pub target_triple: &'static str,
}
