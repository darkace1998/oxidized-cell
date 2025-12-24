//! User profile management
//!
//! Handles PS3 user profiles and settings

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;

/// User profile
#[derive(Debug, Clone)]
pub struct UserProfile {
    /// User ID
    pub user_id: u32,
    /// Username
    pub username: String,
    /// Profile directory path
    pub profile_path: PathBuf,
    /// Settings
    pub settings: HashMap<String, String>,
}

impl UserProfile {
    /// Create a new user profile
    pub fn new(user_id: u32, username: String, profile_path: PathBuf) -> Self {
        Self {
            user_id,
            username,
            profile_path,
            settings: HashMap::new(),
        }
    }

    /// Load profile from directory
    pub fn load(profile_path: PathBuf) -> Result<Self, String> {
        // Extract user ID from path (e.g., /dev_hdd0/home/00000001)
        // User directories are zero-padded 8-digit numbers
        let user_id = profile_path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())  // parse() handles leading zeros correctly
            .ok_or("Invalid user ID in path")?;

        // Load username from settings file
        let username_file = profile_path.join("username");
        let username = std::fs::read_to_string(&username_file)
            .unwrap_or_else(|_| format!("User{:08}", user_id));

        let mut profile = Self {
            user_id,
            username,
            profile_path,
            settings: HashMap::new(),
        };

        // Load settings
        profile.load_settings()?;

        Ok(profile)
    }

    /// Save profile to directory
    pub fn save(&self) -> Result<(), String> {
        // Create profile directory
        std::fs::create_dir_all(&self.profile_path)
            .map_err(|e| format!("Failed to create profile directory: {}", e))?;

        // Save username
        let username_file = self.profile_path.join("username");
        std::fs::write(&username_file, &self.username)
            .map_err(|e| format!("Failed to save username: {}", e))?;

        // Save settings
        self.save_settings()?;

        Ok(())
    }

    /// Load settings from file
    fn load_settings(&mut self) -> Result<(), String> {
        let settings_file = self.profile_path.join("settings.toml");
        if settings_file.exists() {
            let content = std::fs::read_to_string(&settings_file)
                .map_err(|e| format!("Failed to read settings: {}", e))?;

            // Parse settings (simplified, would use TOML in real implementation)
            for line in content.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    self.settings.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }

        Ok(())
    }

    /// Save settings to file
    fn save_settings(&self) -> Result<(), String> {
        let settings_file = self.profile_path.join("settings.toml");

        let content: Vec<String> = self
            .settings
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        std::fs::write(&settings_file, content.join("\n"))
            .map_err(|e| format!("Failed to save settings: {}", e))?;

        Ok(())
    }

    /// Get setting value
    pub fn get_setting(&self, key: &str) -> Option<&String> {
        self.settings.get(key)
    }

    /// Set setting value
    pub fn set_setting(&mut self, key: String, value: String) {
        self.settings.insert(key, value);
    }
}

/// User profile manager
pub struct UserManager {
    /// Active users
    users: RwLock<HashMap<u32, UserProfile>>,
    /// Current user ID
    current_user: RwLock<Option<u32>>,
    /// Base profile directory
    base_path: PathBuf,
}

impl UserManager {
    /// Create a new user manager
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
            current_user: RwLock::new(None),
            base_path,
        }
    }

    /// Create a new user
    pub fn create_user(&self, username: String) -> Result<u32, String> {
        let users = self.users.read();
        
        // Find next available user ID
        let user_id = (1..=999)
            .find(|id| !users.contains_key(id))
            .ok_or("Maximum number of users reached")?;

        drop(users);

        let profile_path = self.base_path.join(format!("{:08}", user_id));
        let profile = UserProfile::new(user_id, username, profile_path);
        
        profile.save()?;

        self.users.write().insert(user_id, profile);

        tracing::info!("Created user: ID={}, username={}", user_id, self.users.read().get(&user_id).unwrap().username);

        Ok(user_id)
    }

    /// Delete a user
    pub fn delete_user(&self, user_id: u32) -> Result<(), String> {
        let mut users = self.users.write();
        
        let profile = users
            .remove(&user_id)
            .ok_or("User not found")?;

        // Delete profile directory
        if profile.profile_path.exists() {
            std::fs::remove_dir_all(&profile.profile_path)
                .map_err(|e| format!("Failed to delete profile directory: {}", e))?;
        }

        // Clear current user if it was deleted
        if *self.current_user.read() == Some(user_id) {
            *self.current_user.write() = None;
        }

        tracing::info!("Deleted user: ID={}", user_id);

        Ok(())
    }

    /// Get user profile
    pub fn get_user(&self, user_id: u32) -> Option<UserProfile> {
        self.users.read().get(&user_id).cloned()
    }

    /// List all users
    pub fn list_users(&self) -> Vec<UserProfile> {
        self.users.read().values().cloned().collect()
    }

    /// Set current user
    pub fn set_current_user(&self, user_id: u32) -> Result<(), String> {
        if !self.users.read().contains_key(&user_id) {
            return Err("User not found".to_string());
        }

        *self.current_user.write() = Some(user_id);
        tracing::info!("Switched to user: ID={}", user_id);

        Ok(())
    }

    /// Get current user ID
    pub fn current_user_id(&self) -> Option<u32> {
        *self.current_user.read()
    }

    /// Get current user profile
    pub fn current_user(&self) -> Option<UserProfile> {
        self.current_user_id().and_then(|id| self.get_user(id))
    }

    /// Load all users from base directory
    pub fn load_users(&self) -> Result<(), String> {
        if !self.base_path.exists() {
            std::fs::create_dir_all(&self.base_path)
                .map_err(|e| format!("Failed to create users directory: {}", e))?;
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.base_path)
            .map_err(|e| format!("Failed to read users directory: {}", e))?;

        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                if let Ok(profile) = UserProfile::load(entry.path()) {
                    self.users.write().insert(profile.user_id, profile);
                }
            }
        }

        tracing::info!("Loaded {} users", self.users.read().len());

        Ok(())
    }

    /// Initialize default user if no users exist
    pub fn init_default_user(&self) -> Result<u32, String> {
        if self.users.read().is_empty() {
            self.create_user("DefaultUser".to_string())
        } else {
            Ok(self.users.read().keys().next().copied().unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_profile_creation() {
        let profile = UserProfile::new(
            1,
            "TestUser".to_string(),
            PathBuf::from("/tmp/test_profile"),
        );

        assert_eq!(profile.user_id, 1);
        assert_eq!(profile.username, "TestUser");
    }

    #[test]
    fn test_user_profile_settings() {
        let mut profile = UserProfile::new(
            1,
            "TestUser".to_string(),
            PathBuf::from("/tmp/test_profile"),
        );

        profile.set_setting("language".to_string(), "en-US".to_string());
        assert_eq!(profile.get_setting("language"), Some(&"en-US".to_string()));
    }

    #[test]
    fn test_user_manager() {
        let temp_dir = std::env::temp_dir().join("test_users");
        let manager = UserManager::new(temp_dir.clone());

        // Clean up before test
        let _ = std::fs::remove_dir_all(&temp_dir);

        let user_id = manager.create_user("TestUser".to_string()).unwrap();
        assert!(user_id > 0);

        let profile = manager.get_user(user_id);
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().username, "TestUser");

        // Clean up after test
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
