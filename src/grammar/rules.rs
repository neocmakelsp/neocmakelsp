//# Copied from cmake-format
//# It shows the lint rules (partly)
//# """
//# Database of lint
//#
//# Categories mirror pylint:
//#
//# * (C) convention, for programming standard violation
//# * (R) refactor, for bad code smell
//# * (W) warning, for cmake specific problems
//# * (E) error, for most probably bugs in the code
//# * (F) fatal, if an error occurred which prevented cmake-lint from doing further
//#       processing.
//# """
//# from __future__ import unicode_literals
//#
//#
//# class Lint(object):
//#   """Basically a named-tuple to store a lint id, message format, and metadata
//#   """
//#
//#   def __init__(self, idstr, msgfmt, description=None, explain=None):
//#     self.idstr = idstr
//#     self.msgfmt = msgfmt
//#     self.description = description
//#     self.explain = explain
//#
//#
//# def get_database():
//#   return {idstr: Lint(idstr, msgfmt, **kwargs)
//#           for idstr, msgfmt, kwargs in LINT_DB}
//#
//#
//# LINT_DB = [
//# # pylint:disable=bad-continuation
//# # noqa: E122
//# (
//# "C0102", "Black listed name \"{:s}\"", {
//# "description": """\
//# Used when the name is listed in the "bad-names" black list.
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# cmake-lint can be customized to help enforce coding guidelines that discourage
//# or forbid use of certain names for variables, functions, etc.. These names are
//# specified with the bad-names option. This message is raised whenever a name is
//# in the list of names defined with the bad-names option.
//# """
//# }), (
//# "C0103", "Invalid {:s} name \"{:s}\" doesn't match `{:s}`", {
//# "description": """\
//# Used when a name doesn't doesn't fit the naming convention associated to its
//# type (function, macro, variable, ...).
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# The naming convention is defined with a regular expression, and the naming
//# convention is satisfied if the name matches the regular expression.
//# """
//# }), (
//# "C0111", "Missing docstring on function or macro declaration", {
//# "description": """\
//# Used when a function or macro is defined without a documentation comment
//# immediately preceding it.
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# So, you've written a some fancy function that makes it "easier" to declare build
//# steps. Congratulations. You probably shouldn't have, but thats OK. Now that you
//# did, how should people use it? What arguments does it take? What are the
//# semantics of those arguments? You should include documentation in a comment
//# block prior to the function declaration with this information.
//# """
//# }), (
//# "C0112", "Empty docstring on function or macro declaration", {
//# "description": """\
//# Used when a function or macro is preceded by an empty comment string, rather
//# that one with useful documentation.
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# Ok so you saw C0111 and figured you'd be clever right? Sorry, no dice. Please
//# include some useful documentation so that code readers know what your
//# function/macro does and how to use it.
//# """
//# }), (
//# "C0113", "Missing {:s} in statement which allows it", {
//# }), (
//# "C0114", "Form discriminator hidden behind variable dereference", {
//# "description": """\
//# Used when a keyword used to discriminate between different forms of a command is
//# hidden behind a variable dereference.
//#
//# This message is implemented by individual command checkers.
//# """,
//# "explain": """\
//# Some cmake commands have very different behavior depending on the presence of
//# a particular keyword (see e.g. the `file` command). And because cmake is a
//# macro language that keyword can actually be held inside a variable. Thus the
//# keyword might not actually be visible to cmake-lint (or humans). In general
//# there is no reason to do this and it really hurts readability since different
//# discriminator keywords yield essentially different commands.
//# """
//# }), (
//# "C0201", "Consider replacing custom parser logic with cmake_parse_arguments", {
//# "description": """\
//# Used when custom parse logic is detected.
//# """
//# }), (
//# "C0202", "Argument name {:s} differs from existing argument only in case", {
//# }), (
//# "C0301", "Line too long ({:d}/{:d})", {
//# "description": """\
//# Used when a line is longer than the limit specified in the line-length
//# option.
//# """,
//# "explain": """\
//# It is a good idea to keep each line within a maximum length to keep it from
//# wrapping past the edge of an editing window. This improves readability and
//# tempers other developers' irritability!
//#
//# The default value of the line-length option is 80, the customary width of a
//# terminal window.
//#
//# Note that the line length and the limit are counted in characters, not in Bytes
//# needed to represent these characters.
//# """
//# }), (
//# "C0303", "Trailing whitespace", {
//# "description": """\
//# Used when a line has one or more whitespace characters directly before the line
//# end character(s).
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# Such trailing whitespace is visually indistinguishable and some editors will
//# trim them.
//# """
//# }), (
//# "C0304", "Final newline missing", {
//# "description": """\
//# Used when a listfile has no line end character(s) on its last line.
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# While cmake itself does not require line end character(s) on the last line,
//# is simply good practice to have it.
//# """
//# }), (
//# "C0305", "{:s} newlines between statements", {
//# }), (
//# "C0306", "Tab-policy violation. Found {:s} but should be {:s}", {
//# }), (
//# "C0307", "Bad indentation:\n{:s}{:s}\n{:s}^----{}\n", {
//# }), (
//# "C0321", "Multiple statements on a single line", {
//# }), (
//# "C0327", "Wrong line ending ({:s})", {
//# "description": """\
//# Used when a line ends with the wrong line ending character. e.g. A line ends
//# with "\\r\\n" when configured for "\\n".
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# While cmake itself does not enforce a particular line ending, it is good
//# practice for a project to be consist with their line endings.
//# """
//# }), (
//# "E0011", "Unrecognized file option {:s}", {
//# "description": """\
//# Used when an unrecognized pragma is encountered.
//# """,
//# "explain": """\
//# cmake-lint allows for some inline comments to suppress warnings (among other
//# things). This lint is emitted if a bad option key is provided in such a pragma
//# """
//# }), (
//# "E0012", "Bad option value {:s}", {
//# "description": """\
//# Used when a cmake-lint pragma is encountered which attempts to alter some option
//# in an invalid way.
//#
//# This message belongs to the basic checker.
//# """,
//# "explain": """\
//# cmake-lint allows for some inline comments to suppress warnings (among other
//# things). This lint is emitted if a bad option is provided to one of these
//# pragmas.
//# """
//# }), (
//# "E0103", "{:s} outside of loop", {
//# "description": """\
//# Used when a break() or continue() statement is used outside a loop.
//#
//# This message belongs to the basic checker.
//# """,
//# }), (
//# "E0108", "Duplicate argument name {:s} in function/macro definition", {
//# }), (
//# "E0109", "Invalid argument name {:s} in function/macro definition", {
//# }), (
//# "E1120", "Missing required positional argument", {
//# "description": """\
//# Used when a positional argument group expecting an exact number of arguments
//# is closed (by a parenthesis) before that number of arguments is found.
//#
//# This message belongs to the basic checker.
//# """,
//# }), (
//# "E1121", "Too many positional arguments", {
//# "description": """\
//# Used when a positional argument is found when no argument group is expected.
//#
//# This message is implemented by individual command checkers
//# """,
//# }), (
//# "E1122", "Duplicate keyword argument {:s}", {
//# "description": """
//# Used when a keyword shows up more than once within an argument group. In
//# general, only COMMAND is allowed more than once.
//# """
//# }), (
//# "E1125", "Missing required keyword argument {:s}", {
//# }), (
//# "E1126", "Invalid form discriminator", {
//# "description": """
//# Used when a keyword used to discriminate between different command forms is
//# omitted.
//# """
//# }), (
//# "R0911", "Too many return statements {:d}/{:d}", {
//#   }), (
//# "R0912", "Too many branches {:d}/{:d}", {
//#   }), (
//# "R0913", "Too many named arguments {:d}/{:d}", {
//#   }), (
//# "R0914", "Too many local variables {:d}/{:d}", {
//#   }), (
//# "R0915", "Too many statements {:d}/{:d}", {
//# }), (
//# "W0101", "Unreachable code", {
//# }), (
//# "W0104", "Use of deprecated command {:s}", {
//# }), (
//# "W0105",
//# "{:s} variable '{:s}' which matches a built-in except for case", {
//# "description": """
//# This warning means that you are using a variable such as,
//# for example, `cmake_cxx_standard` which matches a builtin variable
//# (`CMAKE_CXX_STANDARD`) except for the case. If this was intentional, then it's
//# bad practice as it causes confusion (there are two variables in the namespace
//# with identical name except for case), though it was probably not intentional
//# and you probably aren't assigning to the correct variable.
//#
//# This warning may be emitted for assignment (e.g. `set()` or `list()`) as
//# well as for variable expansion in an argument (e.g. `"${CMAKE_Cxx_STANDARD}"`).
//# """
//# }), (
//# "W0106",
//# "String looks like a variable reference missing an {:s} tag '{:s}'", {
//# })
//# ]

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub enum Level {
    Conversion,
    Refactor,
    Warning,
    Error,
    Fatal,
}

impl Level {
    const fn level_char(&self) -> char {
        match self {
            Self::Conversion => 'C',
            Self::Refactor => 'R',
            Self::Warning => 'W',
            Self::Error => 'E',
            Self::Fatal => 'F',
        }
    }
    fn gen_header(&self, id: &str) -> String {
        format!("[{}{}]", self.level_char(), id)
    }
}

pub trait Rules {
    fn id() -> &'static str;
    fn level() -> Level;
    #[allow(unused)]
    fn description() -> &'static str;
    fn header() -> String {
        Self::level().gen_header(Self::id())
    }
}

pub mod conversion {
    use super::Rules;

    pub const C_UPDOWNCASE: UpDownCase = UpDownCase;
    pub struct UpDownCase;
    impl Rules for UpDownCase {
        fn id() -> &'static str {
            "0308"
        }
        fn level() -> super::Level {
            super::Level::Warning
        }
        fn description() -> &'static str {
            "the variable or the command is deprecated"
        }
    }
    impl UpDownCase {
        pub fn gen_msg(&self, check_info: &str) -> String {
            format!("{} {check_info}", Self::header())
        }
    }

    pub const C_LINE_TOO_LONG: LineTooLong = LineTooLong;
    pub struct LineTooLong;
    impl Rules for LineTooLong {
        fn id() -> &'static str {
            "0301"
        }
        fn level() -> super::Level {
            super::Level::Conversion
        }
        fn description() -> &'static str {
            "the line too long"
        }
    }
    impl LineTooLong {
        pub fn gen_msg(&self, len: usize, max_len: usize) -> String {
            format!(
                "{} Line too long ({len}/{max_len})",
                Self::level().gen_header(Self::id())
            )
        }
    }
}

pub mod error {
    use super::Rules;

    pub const E_GRAMMAR_ERROR: GrammarError = GrammarError;
    pub struct GrammarError;

    impl Rules for GrammarError {
        fn id() -> &'static str {
            "1123"
        }
        fn level() -> super::Level {
            super::Level::Error
        }
        fn description() -> &'static str {
            "Grammar Error"
        }
    }

    impl GrammarError {
        pub fn gen_msg(&self) -> String {
            format!("{} {}", Self::header(), Self::description())
        }
    }

    pub const E_INCLUDE_NO_ARGUMENT: IncludeNoArgument = IncludeNoArgument;
    pub struct IncludeNoArgument;

    impl Rules for IncludeNoArgument {
        fn id() -> &'static str {
            "1124"
        }
        fn level() -> super::Level {
            super::Level::Error
        }
        fn description() -> &'static str {
            "include function need at least one argument"
        }
    }

    impl IncludeNoArgument {
        pub fn gen_msg(&self) -> String {
            format!("{} {}", Self::header(), Self::description())
        }
    }

    pub const E_FILE_OR_DIR_MISSING: FileOrDirMissing = FileOrDirMissing;
    pub struct FileOrDirMissing;

    impl Rules for FileOrDirMissing {
        fn id() -> &'static str {
            "1125"
        }
        fn level() -> super::Level {
            super::Level::Error
        }
        fn description() -> &'static str {
            "File or directory does not exist"
        }
    }

    impl FileOrDirMissing {
        pub fn directory_error_msg(&self, file_name: &str) -> String {
            format!(
                "{} Directory \"{file_name}\" does not exist or is inaccessible",
                Self::header()
            )
        }
        pub fn file_error_msg(&self, file_name: &str) -> String {
            format!(
                "{} File \"{file_name}\" does not exist or is inaccessible",
                Self::header()
            )
        }
    }

    pub const W_INCLUDE_FILE_ERROR: IncludeFileError = IncludeFileError;
    pub struct IncludeFileError;

    impl Rules for IncludeFileError {
        fn id() -> &'static str {
            "1126"
        }
        fn level() -> super::Level {
            super::Level::Error
        }
        fn description() -> &'static str {
            "Error in include file"
        }
    }
    impl IncludeFileError {
        pub fn gen_msg(&self, file_name: &str) -> String {
            format!("{} \"{file_name}\" contains grammar error", Self::header())
        }
    }

    pub const W_INCLUDE_A_DIR: IncludeADir = IncludeADir;
    pub struct IncludeADir;

    impl Rules for IncludeADir {
        fn id() -> &'static str {
            "1127"
        }
        fn level() -> super::Level {
            super::Level::Error
        }
        fn description() -> &'static str {
            "Misincluded a directory"
        }
    }

    impl IncludeADir {
        pub fn gen_msg(&self, dir_name: &str) -> String {
            format!("{} Misincluded a directory \"{dir_name}\"", Self::header())
        }
    }

    pub const W_ADD_SUBDIRECTORY_A_FILE: AddSubdirectoryAFile = AddSubdirectoryAFile;
    pub struct AddSubdirectoryAFile;

    impl Rules for AddSubdirectoryAFile {
        fn id() -> &'static str {
            "1128"
        }
        fn level() -> super::Level {
            super::Level::Error
        }
        fn description() -> &'static str {
            "Mistake add a file as subdirectory"
        }
    }

    impl AddSubdirectoryAFile {
        pub fn gen_msg(&self, file_name: &str) -> String {
            format!(
                "{} Mistake add a file {file_name} as a subdirectory",
                Self::header()
            )
        }
    }

    pub const W_MISSING_PACKAGE: MissingPackage = MissingPackage;
    pub struct MissingPackage;

    impl Rules for MissingPackage {
        fn id() -> &'static str {
            "1129"
        }
        fn level() -> super::Level {
            super::Level::Warning
        }
        fn description() -> &'static str {
            "Cannot find a package"
        }
    }

    impl MissingPackage {
        pub fn gen_msg(&self, package: &str) -> String {
            format!("{} Cannot find package {package}", Self::header())
        }
    }
}

pub mod warn {
    use super::Rules;

    pub const W_DEPRECATED: Deprecated = Deprecated;
    pub struct Deprecated;
    impl Rules for Deprecated {
        fn id() -> &'static str {
            "0104"
        }
        fn level() -> super::Level {
            super::Level::Warning
        }
        fn description() -> &'static str {
            "the variable or the command is deprecated"
        }
    }
    impl Deprecated {
        pub fn gen_msg(&self, cmd_name: &str) -> String {
            format!("{} the {cmd_name} is deprecated", Self::header())
        }
    }
}
