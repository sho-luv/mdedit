Output demo/screenshots/layouts.gif

Set Shell "zsh"
Set FontSize 16
Set Width 1200
Set Height 700
Set Padding 10
Set Theme "Dracula"
Set TypingSpeed 50ms

# Create a sample file
Hide
Type "cat << 'EOF' > /tmp/layout-demo.md"
Enter
Type "# Layout Modes"
Enter
Type ""
Enter
Type "Use **Ctrl+P** to cycle through layouts."
Enter
Type ""
Enter
Type "## Split View"
Enter
Type "Editor on the left, preview on the right."
Enter
Type ""
Enter
Type "## Editor Only"
Enter
Type "Full screen editing when you need focus."
Enter
Type ""
Enter
Type "## Preview Only"
Enter
Type "Read your rendered markdown full screen."
Enter
Type ""
Enter
Type '```python'
Enter
Type 'def hello():'
Enter
Type '    print("Hello from mdedit!")'
Enter
Type '```'
Enter
Type "EOF"
Enter
Show

Type "mdedit /tmp/layout-demo.md"
Enter
Sleep 2s

# Split view (default)
Sleep 1s

# Editor only
Ctrl+p
Sleep 1.5s

# Preview only
Ctrl+p
Sleep 1.5s

# Back to split
Ctrl+p
Sleep 1.5s

Type ":q"
Enter
Sleep 500ms
