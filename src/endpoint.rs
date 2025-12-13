//! Endpoint address parsing for Mutagen sync endpoints.
//!
//! This module provides a type-safe representation of Mutagen endpoint addresses,
//! supporting local paths, SSH remotes, and Docker containers.

use std::path::{Path, PathBuf};

/// Represents a parsed endpoint address.
///
/// Mutagen supports several endpoint formats:
/// - Local paths: `/path/to/dir`, `./relative`, `~/home`
/// - SSH shorthand: `host:/path`, `user@host:/path`
/// - SSH URL: `ssh://[user@]host[:port]/path`
/// - Docker: `docker://container/path`
/// - IPv6: `[::1]:/path`, `[2001:db8::1]:/path`
#[derive(Debug, Clone, PartialEq)]
pub enum EndpointAddress {
    /// A local filesystem path
    Local(PathBuf),
    /// An SSH remote endpoint
    Ssh {
        user: Option<String>,
        host: String,
        port: Option<u16>,
        path: PathBuf,
    },
    /// A Docker container endpoint
    Docker { container: String, path: PathBuf },
}

impl EndpointAddress {
    /// Parse an endpoint string into an EndpointAddress.
    ///
    /// Supports the following formats:
    /// - Local: `/path`, `./relative`, `~/home`, `C:\windows` (Windows)
    /// - SSH shorthand: `host:/path`, `user@host:/path`
    /// - SSH URL: `ssh://[user@]host[:port]/path`
    /// - Docker: `docker://container/path`
    /// - IPv6: `[::1]:/path`, `[2001:db8::1]:/path`
    pub fn parse(s: &str) -> Self {
        // 1. Check for URL schemes
        if let Some(rest) = s.strip_prefix("ssh://") {
            return Self::parse_ssh_url(rest);
        }
        if let Some(rest) = s.strip_prefix("docker://") {
            return Self::parse_docker_url(rest);
        }

        // 2. Check for IPv6 with brackets: [addr]:/path
        if s.starts_with('[') {
            if let Some(bracket_end) = s.find(']') {
                let host = &s[1..bracket_end];
                let rest = &s[bracket_end + 1..];
                if let Some(path) = rest.strip_prefix(':') {
                    return EndpointAddress::Ssh {
                        user: None,
                        host: host.to_string(),
                        port: None,
                        path: PathBuf::from(path),
                    };
                }
            }
        }

        // 3. Check for Windows drive letters: C:\path or C:/path
        // A single letter followed by : and then \ or / is a Windows path
        let chars: Vec<char> = s.chars().collect();
        if chars.len() >= 3
            && chars[0].is_ascii_alphabetic()
            && chars[1] == ':'
            && (chars[2] == '\\' || chars[2] == '/')
        {
            return EndpointAddress::Local(PathBuf::from(s));
        }

        // 4. Check for SSH shorthand: host:/path or user@host:/path
        // Look for ':' that's followed by '/' (to distinguish from Windows paths)
        if let Some(colon_pos) = s.find(':') {
            let after_colon = &s[colon_pos + 1..];
            // SSH paths typically start with / or ~ after the colon
            if after_colon.starts_with('/') || after_colon.starts_with('~') {
                let host_part = &s[..colon_pos];
                let path = &s[colon_pos + 1..];

                // Check for user@host format
                if let Some(at_pos) = host_part.find('@') {
                    let user = &host_part[..at_pos];
                    let host = &host_part[at_pos + 1..];
                    return EndpointAddress::Ssh {
                        user: Some(user.to_string()),
                        host: host.to_string(),
                        port: None,
                        path: PathBuf::from(path),
                    };
                } else {
                    return EndpointAddress::Ssh {
                        user: None,
                        host: host_part.to_string(),
                        port: None,
                        path: PathBuf::from(path),
                    };
                }
            }
        }

        // 5. Default: local path
        EndpointAddress::Local(PathBuf::from(s))
    }

    /// Parse an SSH URL (without the ssh:// prefix)
    fn parse_ssh_url(s: &str) -> Self {
        // Format: [user@]host[:port]/path
        let (user_host_port, path) = if let Some(slash_pos) = s.find('/') {
            (&s[..slash_pos], &s[slash_pos..])
        } else {
            (s, "/")
        };

        let (user, host_port) = if let Some(at_pos) = user_host_port.find('@') {
            (
                Some(user_host_port[..at_pos].to_string()),
                &user_host_port[at_pos + 1..],
            )
        } else {
            (None, user_host_port)
        };

        let (host, port) = if let Some(colon_pos) = host_port.find(':') {
            let port_str = &host_port[colon_pos + 1..];
            let port = port_str.parse().ok();
            (&host_port[..colon_pos], port)
        } else {
            (host_port, None)
        };

        EndpointAddress::Ssh {
            user,
            host: host.to_string(),
            port,
            path: PathBuf::from(path),
        }
    }

    /// Parse a Docker URL (without the docker:// prefix)
    fn parse_docker_url(s: &str) -> Self {
        // Format: container/path
        if let Some(slash_pos) = s.find('/') {
            EndpointAddress::Docker {
                container: s[..slash_pos].to_string(),
                path: PathBuf::from(&s[slash_pos..]),
            }
        } else {
            EndpointAddress::Docker {
                container: s.to_string(),
                path: PathBuf::from("/"),
            }
        }
    }

    /// Expand tilde (~) in local paths to the user's home directory.
    ///
    /// Only expands tilde for Local endpoints. Remote endpoints preserve
    /// the tilde as it will be expanded by the remote shell.
    pub fn expand_tilde(self) -> Self {
        match self {
            EndpointAddress::Local(path) => {
                let path_str = path.to_string_lossy();
                if path_str.starts_with('~') {
                    if let Some(home) = dirs::home_dir() {
                        let expanded = if path_str == "~" {
                            home
                        } else if let Some(rest) = path_str.strip_prefix("~/") {
                            home.join(rest)
                        } else {
                            // ~username format - not supported, return as-is
                            path
                        };
                        return EndpointAddress::Local(expanded);
                    }
                }
                EndpointAddress::Local(path)
            }
            // Remote paths: tilde is handled by the remote shell
            other => other,
        }
    }

    /// Returns the path component of the endpoint.
    pub fn path(&self) -> &Path {
        match self {
            EndpointAddress::Local(p) => p,
            EndpointAddress::Ssh { path, .. } => path,
            EndpointAddress::Docker { path, .. } => path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Local path tests
    #[test]
    fn test_parse_absolute_path() {
        let ep = EndpointAddress::parse("/home/user/project");
        assert_eq!(
            ep,
            EndpointAddress::Local(PathBuf::from("/home/user/project"))
        );
    }

    #[test]
    fn test_parse_relative_path() {
        let ep = EndpointAddress::parse("./relative/path");
        assert_eq!(ep, EndpointAddress::Local(PathBuf::from("./relative/path")));
    }

    #[test]
    fn test_parse_tilde_path() {
        let ep = EndpointAddress::parse("~/projects/myapp");
        assert_eq!(
            ep,
            EndpointAddress::Local(PathBuf::from("~/projects/myapp"))
        );
    }

    #[test]
    fn test_parse_windows_path() {
        let ep = EndpointAddress::parse("C:\\Users\\test\\project");
        assert_eq!(
            ep,
            EndpointAddress::Local(PathBuf::from("C:\\Users\\test\\project"))
        );
    }

    #[test]
    fn test_parse_windows_path_forward_slash() {
        let ep = EndpointAddress::parse("D:/Projects/app");
        assert_eq!(ep, EndpointAddress::Local(PathBuf::from("D:/Projects/app")));
    }

    // SSH shorthand tests
    #[test]
    fn test_parse_ssh_shorthand_simple() {
        let ep = EndpointAddress::parse("myhost:/path/to/dir");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: None,
                host: "myhost".to_string(),
                port: None,
                path: PathBuf::from("/path/to/dir"),
            }
        );
    }

    #[test]
    fn test_parse_ssh_shorthand_with_user() {
        let ep = EndpointAddress::parse("user@myhost:/path/to/dir");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: Some("user".to_string()),
                host: "myhost".to_string(),
                port: None,
                path: PathBuf::from("/path/to/dir"),
            }
        );
    }

    #[test]
    fn test_parse_ssh_shorthand_with_tilde() {
        let ep = EndpointAddress::parse("server:~/code/project");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: None,
                host: "server".to_string(),
                port: None,
                path: PathBuf::from("~/code/project"),
            }
        );
    }

    // SSH URL tests
    #[test]
    fn test_parse_ssh_url_simple() {
        let ep = EndpointAddress::parse("ssh://myhost/path/to/dir");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: None,
                host: "myhost".to_string(),
                port: None,
                path: PathBuf::from("/path/to/dir"),
            }
        );
    }

    #[test]
    fn test_parse_ssh_url_with_user() {
        let ep = EndpointAddress::parse("ssh://user@myhost/path/to/dir");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: Some("user".to_string()),
                host: "myhost".to_string(),
                port: None,
                path: PathBuf::from("/path/to/dir"),
            }
        );
    }

    #[test]
    fn test_parse_ssh_url_with_port() {
        let ep = EndpointAddress::parse("ssh://myhost:2222/path/to/dir");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: None,
                host: "myhost".to_string(),
                port: Some(2222),
                path: PathBuf::from("/path/to/dir"),
            }
        );
    }

    #[test]
    fn test_parse_ssh_url_with_user_and_port() {
        let ep = EndpointAddress::parse("ssh://admin@server:22/home/admin/project");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: Some("admin".to_string()),
                host: "server".to_string(),
                port: Some(22),
                path: PathBuf::from("/home/admin/project"),
            }
        );
    }

    // IPv6 tests
    #[test]
    fn test_parse_ipv6_localhost() {
        let ep = EndpointAddress::parse("[::1]:/path/to/dir");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: None,
                host: "::1".to_string(),
                port: None,
                path: PathBuf::from("/path/to/dir"),
            }
        );
    }

    #[test]
    fn test_parse_ipv6_full_address() {
        let ep = EndpointAddress::parse("[2001:db8::1]:/var/data");
        assert_eq!(
            ep,
            EndpointAddress::Ssh {
                user: None,
                host: "2001:db8::1".to_string(),
                port: None,
                path: PathBuf::from("/var/data"),
            }
        );
    }

    // Docker tests
    #[test]
    fn test_parse_docker_url() {
        let ep = EndpointAddress::parse("docker://mycontainer/app/data");
        assert_eq!(
            ep,
            EndpointAddress::Docker {
                container: "mycontainer".to_string(),
                path: PathBuf::from("/app/data"),
            }
        );
    }

    #[test]
    fn test_parse_docker_url_no_path() {
        let ep = EndpointAddress::parse("docker://mycontainer");
        assert_eq!(
            ep,
            EndpointAddress::Docker {
                container: "mycontainer".to_string(),
                path: PathBuf::from("/"),
            }
        );
    }

    // Tilde expansion tests
    #[test]
    fn test_expand_tilde_local() {
        let ep = EndpointAddress::parse("~/projects/myapp");
        let expanded = ep.expand_tilde();
        if let EndpointAddress::Local(path) = expanded {
            // Should be expanded to home directory
            assert!(!path.to_string_lossy().starts_with('~'));
            assert!(path.to_string_lossy().contains("projects/myapp"));
        } else {
            panic!("Expected Local endpoint");
        }
    }

    #[test]
    fn test_expand_tilde_just_home() {
        let ep = EndpointAddress::parse("~");
        let expanded = ep.expand_tilde();
        if let EndpointAddress::Local(path) = expanded {
            assert!(!path.to_string_lossy().starts_with('~'));
        } else {
            panic!("Expected Local endpoint");
        }
    }

    #[test]
    fn test_expand_tilde_remote_unchanged() {
        let ep = EndpointAddress::parse("server:~/code/project");
        let expanded = ep.expand_tilde();
        // Remote paths should NOT have tilde expanded
        if let EndpointAddress::Ssh { path, .. } = expanded {
            assert!(path.to_string_lossy().starts_with('~'));
        } else {
            panic!("Expected Ssh endpoint");
        }
    }

    #[test]
    fn test_expand_tilde_absolute_unchanged() {
        let ep = EndpointAddress::parse("/absolute/path");
        let expanded = ep.expand_tilde();
        assert_eq!(
            expanded,
            EndpointAddress::Local(PathBuf::from("/absolute/path"))
        );
    }

    // Accessor tests
    #[test]
    fn test_path_accessor() {
        let local = EndpointAddress::parse("/home/user");
        assert_eq!(local.path(), Path::new("/home/user"));

        let ssh = EndpointAddress::parse("host:/remote/path");
        assert_eq!(ssh.path(), Path::new("/remote/path"));

        let docker = EndpointAddress::parse("docker://container/app");
        assert_eq!(docker.path(), Path::new("/app"));
    }

}
