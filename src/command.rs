use std::collections::HashMap;

/// Command prefix types supported by the bot
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandType {
    Query,      // \q - query commands
    Image,      // \img - image commands
    Execute,    // \exec - execution commands
    Help,       // \help - help commands
    Unknown,    // Unrecognized command
}

impl CommandType {
    /// Parse command type from prefix
    pub fn from_prefix(prefix: &str) -> Self {
        match prefix.to_lowercase().as_str() {
            "\\q" | "/q" => CommandType::Query,
            "\\img" | "/img" => CommandType::Image,
            "\\exec" | "/exec" => CommandType::Execute,
            "\\help" | "/help" => CommandType::Help,
            _ => CommandType::Unknown,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &str {
        match self {
            CommandType::Query => "query",
            CommandType::Image => "image",
            CommandType::Execute => "execute",
            CommandType::Help => "help",
            CommandType::Unknown => "unknown",
        }
    }
}

/// Parsed command structure
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub command_type: CommandType,
    pub raw_text: String,
    pub arguments: String,
    pub metadata: HashMap<String, String>,
}

impl ParsedCommand {
    /// Parse a message into a command
    /// 
    /// # Examples
    /// ```
    /// let cmd = ParsedCommand::parse("\\q iss status");
    /// assert_eq!(cmd.command_type, CommandType::Query);
    /// assert_eq!(cmd.arguments, "iss status");
    /// ```
    pub fn parse(text: &str) -> Self {
        let trimmed = text.trim();
        
        // Check if starts with a command prefix
        if let Some((prefix, rest)) = Self::extract_prefix(trimmed) {
            let command_type = CommandType::from_prefix(prefix);
            let arguments = rest.trim().to_string();
            
            let mut metadata = HashMap::new();
            metadata.insert("command_prefix".to_string(), prefix.to_string());
            metadata.insert("command_type".to_string(), command_type.as_str().to_string());
            
            // Add argument count
            let arg_count = if arguments.is_empty() {
                0
            } else {
                arguments.split_whitespace().count()
            };
            metadata.insert("arg_count".to_string(), arg_count.to_string());
            
            ParsedCommand {
                command_type,
                raw_text: trimmed.to_string(),
                arguments,
                metadata,
            }
        } else {
            // Not a command, treat as regular message
            ParsedCommand {
                command_type: CommandType::Unknown,
                raw_text: trimmed.to_string(),
                arguments: String::new(),
                metadata: HashMap::new(),
            }
        }
    }

    /// Extract prefix and rest of the message
    fn extract_prefix(text: &str) -> Option<(&str, &str)> {
        // Check for backslash commands
        if text.starts_with('\\') {
            if let Some(space_pos) = text.find(char::is_whitespace) {
                return Some((&text[..space_pos], &text[space_pos..]));
            } else {
                return Some((text, ""));
            }
        }
        
        // Check for forward slash commands
        if text.starts_with('/') {
            if let Some(space_pos) = text.find(char::is_whitespace) {
                return Some((&text[..space_pos], &text[space_pos..]));
            } else {
                return Some((text, ""));
            }
        }
        
        None
    }

    /// Check if this is a valid command (not Unknown)
    pub fn is_command(&self) -> bool {
        self.command_type != CommandType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_command() {
        let cmd = ParsedCommand::parse("\\q iss status");
        assert_eq!(cmd.command_type, CommandType::Query);
        assert_eq!(cmd.arguments, "iss status");
        assert!(cmd.is_command());
    }

    #[test]
    fn test_image_command() {
        let cmd = ParsedCommand::parse("/img cat");
        assert_eq!(cmd.command_type, CommandType::Image);
        assert_eq!(cmd.arguments, "cat");
    }

    #[test]
    fn test_no_command() {
        let cmd = ParsedCommand::parse("Hello world");
        assert_eq!(cmd.command_type, CommandType::Unknown);
        assert!(!cmd.is_command());
    }

    #[test]
    fn test_command_no_args() {
        let cmd = ParsedCommand::parse("\\help");
        assert_eq!(cmd.command_type, CommandType::Help);
        assert_eq!(cmd.arguments, "");
    }
}
