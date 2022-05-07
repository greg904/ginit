//! Backups are very important. Unfortunately, they are easy to get wrong.
//!
//! ## Requirements
//!
//! First, these are the requirements for the backups:
//! * they should be automatic
//! * they should be stored in multiple different locations (including the
//!   host computer)
//! * they should not be large because online storage is not cheap
//! * they should be taken at least every day when the computer is used
//! * at least 2 years worth of backup should be kept
//!
//! ## Implementation
//!
//! Backups are stored on remotes as files on a single directory. The file
//! names are either `<year>-<month>-<day>-full.zst.enc` or
//! `<year>-<month>-<day>-from-<year>-<month>-<day>.zst.enc` depending on the
//! type of backup. Indeed, backups can either be full backups or incremental
//! with a list of changes from a previous backup. This is made possible thanks
//! to the btrfs filesystem which can calculate these diffs efficiently.
