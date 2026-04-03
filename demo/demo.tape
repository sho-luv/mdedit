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
Type "- System clipboard support"
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
Type "> Edit markdown. See it rendered. In your terminal."
Enter
Type "EOF"
Enter
Sleep 500ms

# Open mdedit
Type "mdedit /tmp/demo.md"
Enter
Sleep 2s

# Navigate down to show content
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
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
Type@50ms ""
Sleep 500ms

# Back to normal mode
Escape
Sleep 1s

# Save
Type ":w"
Enter
Sleep 1s

# Toggle to preview only
Ctrl+p
Sleep 2s

# Toggle back to split
Ctrl+p
Sleep 1s

# Toggle to editor only
Ctrl+p
Sleep 1s

# Back to split
Ctrl+p
Sleep 2s

# Quit
Type ":q"
Enter
Sleep 500ms
