#[cfg(feature = "chrono")]
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use git2;

#[cfg(feature = "chrono")]
use super::DAYS_UNTIL_STALE;
use super::{signature::Signature, Repository};
use error::{Error, ErrorKind};

/// Information about a commit to the Git repository
#[derive(Debug)]
pub struct Commit {
    /// ID (i.e. SHA-1 hash) of the latest commit
    pub commit_id: String,

    /// Information about the author of a commit
    pub author: String,

    /// Summary message for the commit
    pub summary: String,

    /// Commit time in number of seconds since the UNIX epoch
    #[cfg(feature = "chrono")]
    pub time: DateTime<Utc>,

    /// Signature on the commit (mandatory for Repository::fetch)
    // TODO: actually verify signatures
    pub signature: Option<Signature>,

    /// Signed data to verify along with this commit
    signed_data: Option<Vec<u8>>,
}

impl Commit {
    /// Get information about HEAD
    pub(crate) fn from_repo_head(repo: &Repository) -> Result<Self, Error> {
        let head = repo.repo.head()?;

        let oid = head.target().ok_or_else(|| {
            err!(
                ErrorKind::Repo,
                "no ref target for: {}",
                repo.path.display()
            )
        })?;

        let commit_id = oid.to_string();
        let commit_object = repo.repo.find_object(oid, Some(git2::ObjectType::Commit))?;
        let commit = commit_object.as_commit().unwrap();
        let author = commit.author().to_string();

        let summary = commit
            .summary()
            .ok_or_else(|| err!(ErrorKind::Repo, "no commit summary for {}", commit_id))?
            .to_owned();

        let (signature, signed_data) = match repo.repo.extract_signature(&oid, None) {
            Ok((sig, data)) => (Some(Signature::new(&*sig)?), Some(Vec::from(&*data))),
            _ => (None, None),
        };

        #[cfg(feature = "chrono")]
        let time = DateTime::from_utc(
            NaiveDateTime::from_timestamp(commit.time().seconds(), 0),
            Utc,
        );

        Ok(Commit {
            commit_id,
            author,
            summary,
            #[cfg(feature = "chrono")]
            time,
            signature,
            signed_data,
        })
    }

    /// Get the raw bytes to be verified when verifying a commit signature
    pub fn raw_signed_bytes(&self) -> Option<&[u8]> {
        self.signed_data.as_ref().map(|bytes| bytes.as_ref())
    }

    /// Reset the repository's state to match this commit
    #[cfg(feature = "chrono")]
    pub(crate) fn reset(&self, repo: &Repository) -> Result<(), Error> {
        let commit_object = repo.repo.find_object(
            git2::Oid::from_str(&self.commit_id).unwrap(),
            Some(git2::ObjectType::Commit),
        )?;

        // Reset the state of the repository to the latest commit
        repo.repo
            .reset(&commit_object, git2::ResetType::Hard, None)?;

        Ok(())
    }

    /// Determine if the repository is fresh or stale (i.e. has it recently been committed to)
    #[cfg(feature = "chrono")]
    pub(crate) fn ensure_fresh(&self) -> Result<(), Error> {
        let fresh_after_date = Utc::now()
            .checked_sub_signed(Duration::days(DAYS_UNTIL_STALE as i64))
            .unwrap();

        if self.time > fresh_after_date {
            Ok(())
        } else {
            fail!(
                ErrorKind::Repo,
                "stale repo: not updated for {} days (last commit: {:?})",
                DAYS_UNTIL_STALE,
                self.time
            )
        }
    }
}
