# The four views, cycled with Ctrl+P: live, split, editor only, preview only.
#   cargo build --release && vhs demo/screenshots/layouts.tape
Output demo/screenshots/layouts.gif

Set Shell "bash"
Set FontSize 15
Set Width 1200
Set Height 720
Set Padding 12
Set TypingSpeed 45ms

Env HOME "/tmp/mdedit-demo"
Env COLORTERM "truecolor"

Hide
Type 'mkdir -p /tmp/mdedit-demo && cp demo/sample.md /tmp/mdedit-demo/notes.md && export PATH="$PWD/target/release:$PATH" && cd /tmp/mdedit-demo && clear'
Enter
Sleep 500ms
Type "mdedit notes.md"
Enter
Sleep 1.5s
Type "jjjjjjjjjj"
Sleep 300ms
Show

# Live view (default)
Sleep 2.2s

# Split view
Ctrl+p
Sleep 2.2s

# Editor only
Ctrl+p
Sleep 2.2s

# Preview only
Ctrl+p
Sleep 2.2s

# Back to live
Ctrl+p
Sleep 2s

Hide
Type ":q!"
Enter
