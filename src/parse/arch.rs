use std::str::FromStr;

use failure::bail;
use failure::Error;

macro_rules! strum {
    ($name:ident, $($variant:ident($str:expr),)*) => {
        #[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
        pub enum $name {
            $($variant,)*
        }

        impl FromStr for $name {
            type Err = Error;
            fn from_str(from: &str) -> Result<$name, Error> {
                match from {
                    $($str => Ok($name::$variant), )*
                    other => bail!("no {}: {:?}", stringify!($name), other),
                }
            }
        }
    }
}

strum!(
    Kernel,
    Aix("aix"),
    Darwin("darwin"),
    DragonflyBsd("dragonflybsd"),
    FreeBsd("freebsd"),
    Hurd("hurd"),
    KFreeBsd("kfreebsd"),
    KNetBsd("knetbsd"),
    KOpenSolaris("kopensolaris"),
    Linux("linux"),
    Mint("mint"),
    MuslLinux("musl-linux"),
    NetBsd("netbsd"),
    OpenBsd("openbsd"),
    Solaris("solaris"),
    UcLibcLinux("uclibc-linux"),
    UcLinux("uclinux"),
);

strum!(
    Cpu,
    Alpha("alpha"),
    Amd64("amd64"),
    Arm("arm"),
    Arm64("arm64"),
    Arm64ilp32("arm64ilp32"),
    Armeb("armeb"),
    Armel("armel"),
    Armhf("armhf"),
    Avr32("avr32"),
    Hppa("hppa"),
    I386("i386"),
    Ia64("ia64"),
    Lpia("lpia"),
    M32r("m32r"),
    M68k("m68k"),
    Mips("mips"),
    Mips64("mips64"),
    Mips64el("mips64el"),
    Mips64r6("mips64r6"),
    Mips64r6el("mips64r6el"),
    Mipsel("mipsel"),
    Mipsn32("mipsn32"),
    Mipsn32el("mipsn32el"),
    Mipsn32r6("mipsn32r6"),
    Mipsn32r6el("mipsn32r6el"),
    Mipsr6("mipsr6"),
    Mipsr6el("mipsr6el"),
    Nios2("nios2"),
    Or1k("or1k"),
    Powerpc("powerpc"),
    Powerpcel("powerpcel"),
    Powerpcspe("powerpcspe"),
    Ppc64("ppc64"),
    Ppc64el("ppc64el"),
    Riscv64("riscv64"),
    S390("s390"),
    S390x("s390x"),
    Sh3("sh3"),
    Sh3eb("sh3eb"),
    Sh4("sh4"),
    Sh4eb("sh4eb"),
    Sparc("sparc"),
    Sparc64("sparc64"),
    Tilegx("tilegx"),
    X32("x32"),
);
