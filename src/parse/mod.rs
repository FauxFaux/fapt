//! These types are used to represent a [crate::parse::Package].

mod arch;
mod bin;
mod deps;
mod ident;
mod pkg;
mod src;
mod vcs;

pub use self::arch::Arch;
pub use self::arch::Cpu;
pub use self::arch::Kernel;
pub use self::bin::Binary;
pub use self::deps::Constraint;
pub use self::deps::ConstraintOperator;
pub use self::deps::Dependency;
pub use self::deps::SingleDependency;
pub use self::ident::Identity;
pub use self::pkg::Package;
pub use self::pkg::PackageType;
pub use self::pkg::Priority;
pub use self::src::Source;
pub use self::src::SourceArchive;
pub use self::src::SourceBinary;
pub use self::src::SourceFormat;
pub use self::vcs::Vcs;
pub use self::vcs::VcsTag;
pub use self::vcs::VcsType;
