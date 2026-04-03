Output demo/screenshots/settings.gif

Set Shell "zsh"
Set FontSize 16
Set Width 1200
Set Height 700
Set Padding 10
Set Theme "Dracula"
Set TypingSpeed 50ms

# Create a sample file
Hide
Type "cat << 'EOF' > /tmp/settings-demo.md"
Enter
Type "# Theme Demo"
Enter
Type ""
Enter
Type "This shows the **live settings panel** in mdedit."
Enter
Type ""
Enter
Type "## Features"
Enter
Type ""
Enter
Type "- Switch themes on the fly"
Enter
Type "- Save your preferences"
Enter
Type "- Reset to defaults"
Enter
Type "EOF"
Enter
Show

# Open mdedit
Type "mdedit /tmp/settings-demo.md"
Enter
Sleep 1.5s

# Open settings
Type ":set"
Enter
Sleep 1.5s

# Cycle through themes
Right
Sleep 1s
Right
Sleep 1s
Right
Sleep 1s
Right
Sleep 1.5s

# Go back to ocean
Left
Sleep 0.5s
Left
Sleep 0.5s
Left
Sleep 1s

# Close and quit
Escape
Sleep 0.5s
Type ":q"
Enter
Sleep 500ms
