//! The real [`GitHubPort`]: GitHub asked and changed through gh, whose sign-in is the person's - rhizoid keeps no
//! token of its own (concept `auth-via-gh`). The only place rhizoid runs gh.

use rhizoid_core::{GitHubPort, MergeOutcome, PortError};
use std::process::{Command, Stdio};

/// gh, never prompting.
pub struct GhCli;

impl GhCli {
    fn gh(&self, args: &[&str]) -> Result<(bool, String, String), PortError> {
        let out = Command::new("gh")
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| PortError(format!("cannot run gh: {e}")))?;
        Ok((
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }

    fn ok(&self, args: &[&str]) -> Result<String, PortError> {
        let (ok, out, err) = self.gh(args)?;
        if ok {
            Ok(out)
        } else {
            Err(PortError(format!("gh {}: {err}", args.join(" "))))
        }
    }
}

impl GitHubPort for GhCli {
    fn fork(&self, source: &str, org: &str) -> Result<String, PortError> {
        let me = self.ok(&["api", "user", "--jq", ".login"])?;
        // every branch comes with the fork: a cutting may track one that is not the default
        let mut args = vec!["repo", "fork", source, "--clone=false"];
        if !org.eq_ignore_ascii_case(&me) {
            args.extend(["--org", org]);
        }
        self.ok(&args)?;
        let name = source.rsplit('/').next().unwrap_or(source);
        Ok(format!("{org}/{name}"))
    }

    fn merge_upstream(&self, fork: &str, branch: &str) -> Result<MergeOutcome, PortError> {
        // merge-upstream merges when the fork has commits its parent lacks: tracking only ever fast-forwards, so
        // that is refused before it is asked
        let parent = self.ok(&["api", &format!("repos/{fork}"), "--jq", ".parent.owner.login"])?;
        let compare = format!("repos/{fork}/compare/{parent}:{branch}...{branch}");
        let ahead = self.ok(&["api", &compare, "--jq", ".ahead_by"])?;
        if ahead.trim() != "0" {
            return Err(PortError(format!(
                "{fork} {branch} has {ahead} commits its parent lacks: changed by hand - merge-upstream would merge, so it is not asked"
            )));
        }
        let path = format!("repos/{fork}/merge-upstream");
        let field = format!("branch={branch}");
        let merged = self.ok(&["api", "-X", "POST", &path, "-f", &field, "--jq", ".merge_type"])?;
        Ok(if merged == "none" {
            MergeOutcome::AlreadyUpToDate
        } else if merged != "fast-forward" {
            return Err(PortError(format!(
                "merge-upstream on {fork} {branch} answered {merged}, not a fast-forward"
            )));
        } else {
            let head =
                self.ok(&["api", &format!("repos/{fork}/commits/{branch}"), "--jq", ".sha"])?;
            MergeOutcome::FastForwarded { new_commit: head }
        })
    }

    fn repo_exists(&self, full_name: &str) -> Result<bool, PortError> {
        Ok(self.gh(&["repo", "view", full_name, "--json", "name"])?.0)
    }

    fn parent(&self, full_name: &str) -> Result<Option<String>, PortError> {
        let p =
            self.ok(&["api", &format!("repos/{full_name}"), "--jq", ".parent.full_name // empty"])?;
        Ok((!p.is_empty()).then_some(p))
    }

    fn default_branch(&self, full_name: &str) -> Result<String, PortError> {
        self.ok(&["api", &format!("repos/{full_name}"), "--jq", ".default_branch"])
    }
}
