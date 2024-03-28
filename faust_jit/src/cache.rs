use chksum_sha1 as sha1;
use sha1::{Chksumable, SHA1};
use std::fs;
use std::path::{Path, PathBuf};

/// A folder where to store results of _deterministic_ computations. Light to
/// clone
#[derive(Clone)]
pub struct Cache {
    root: PathBuf,
}

pub struct ResultId(sha1::Digest);

pub enum CacheCheck {
    Hit(PathBuf),
    Miss(CacheWriter),
}

impl Cache {
    /// Open a cache in a folder, ensuring it exists
    pub fn new(cache_folder: PathBuf) -> Self {
        fs::create_dir_all(&cache_folder).expect("Cache root folder couldn't be created");
        Self { root: cache_folder }
    }

    /// Hash the inputs of some computation. T can just be &Path
    pub fn hash_input<T: Chksumable + Clone>(
        mut input: T,
        other_inputs: &[T],
    ) -> Result<ResultId, sha1::Error> {
        let mut sha1 = SHA1::new();
        input.chksum_with(&mut sha1)?;
        for p in other_inputs {
            p.clone().chksum_with(&mut sha1)?;
        }
        Ok(ResultId(sha1.digest()))
    }

    /// Query if a computation's result is already in cache. If not, returns a
    /// way to write the result
    pub fn query(&self, ResultId(digest): ResultId) -> CacheCheck {
        let mut final_dir = self.root.clone();
        final_dir.push(digest.to_hex_lowercase());
        if final_dir.exists() {
            CacheCheck::Hit(final_dir)
        } else {
            let mut temp_dir = self.root.clone();
            temp_dir.push(format!(
                "tmp-{}-{}",
                digest.to_hex_lowercase(),
                rand::random::<u32>()
            ));
            CacheCheck::Miss(CacheWriter {
                temp_dir,
                final_dir,
            })
        }
    }
}

// A lock-less mechanism for the cache. Files aren't written to their final
// destinations, they are written to a unique temp folder, which is then renamed
// when the CacheWriter is dropped. Final destinations are indexed by the hash
// of the deterministic computation that produced them, so in case they end up
// being overwritten, it would be with the exact same contents
pub struct CacheWriter {
    temp_dir: PathBuf,
    final_dir: PathBuf,
}

impl CacheWriter {
    pub fn with_dest_folder<T>(&self, f: impl FnOnce(&Path) -> T) -> T {
        fs::create_dir_all(&self.temp_dir).expect(&format!(
            "Cache temp folder {:?} couldn't be created",
            &self.temp_dir
        ));

        let result = f(&self.temp_dir);

        if self.final_dir.exists() {
            // Result has been created elsewhere in the meatime, we just
            // remove the temp dir:
            fs::remove_dir_all(&self.temp_dir)
                .expect(&format!("Couldn't remove temp dir {:?}", self.temp_dir));
        } else {
            fs::rename(&self.temp_dir, &self.final_dir).expect(&format!(
                "Couldn't rename temp dir {:?} as {:?}",
                self.temp_dir, self.final_dir
            ));
        }

        result
    }
}
