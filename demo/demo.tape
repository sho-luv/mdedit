# mdedit demo - terminal markdown editor with live preview
Output demo/mdedit-demo.gif

Set Shell "zsh"
Set FontSize 14
Set Width 1400
Set Height 800
Set Padding 10
Set Theme "Dracula"
Set TypingSpeed 50ms

# Create a sample markdown file
Hide
Type "cat << 'EOF' > /tmp/demo.md"
Enter
Type "# Welcome to mdedit"
Enter
Type ""
Enter
Type "A terminal markdown editor with **live preview**."
Enter
Type ""
Enter
Type "## Features"
Enter
Type ""
Enter
Type "- Side-by-side editing"
Enter
Type "- Vim keybindings"
Enter
Type "- Syntax highlighting"
Enter
Type "- Render profiles (GitHub, Obsidian, CommonMark)"
Enter
Type ""
Enter
Type "## Code Example"
Enter
Type ""
Enter
Type '```rust'
Enter
Type 'fn main() {'
Enter
Type '    println!("Hello, mdedit!");'
Enter
Type '}'
Enter
Type '```'
Enter
Type ""
Enter
Type "| Feature | mdedit | VS Code |"
Enter
Type "|---------|--------|---------|"
Enter
Type "| Terminal | Yes | No |"
Enter
Type "| SSH | Yes | No |"
Enter
Type "| Startup | <50ms | 3s+ |"
Enter
Type ""
Enter
Type "> [!NOTE]"
Enter
Type "> Callouts render with colored borders and icons."
Enter
Type ""
Enter
Type "> Edit markdown. See it rendered. In your terminal."
Enter
Type "EOF"
Enter
Show

# Open mdedit
Type "mdedit /tmp/demo.md"
Enter
Sleep 2s

# Navigate down to show content and sync indicator
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 2s

# Go back to top
Type "gg"
Sleep 1s

# Enter insert mode and add text
Type "G"
Sleep 500ms
Type "o"
Sleep 300ms
Type "- [x] Task lists work too!"
Sleep 1s

# Back to normal mode
Escape
Sleep 1s

# Open settings panel
Type ":set"
Enter
Sleep 1.5s

# Navigate to Preview profile
Down
Sleep 500ms
Down
Sleep 500ms

# Cycle through profiles
Right
Sleep 1s
Right
Sleep 1s
Left
Sleep 1s
Left
Sleep 1s

# Close settings
Escape
Sleep 1s

# Save
Type ":w"
Enter
Sleep 1s

# Toggle layout modes
Ctrl+p
Sleep 1.5s
Ctrl+p
Sleep 1.5s
Ctrl+p
Sleep 1.5s

# Quit
Type ":q"
Enter
Sleep 500ms
