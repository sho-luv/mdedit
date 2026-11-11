# The settings panel (F2 or :set): themes and preview styles, applied live.
#   cargo build --release && vhs demo/screenshots/settings.tape
Output demo/screenshots/settings.gif

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
Type "mdedit --view split notes.md"
Enter
Sleep 1.5s
Type "jjjjjjjjjj"
Sleep 300ms
Show
Sleep 1s

# Open settings
Type ":set"
Sleep 400ms
Enter
Sleep 1.5s

# Cycle themes
Right
Sleep 1.4s
Right
Sleep 1.4s
Right
Sleep 1.4s
Right
Sleep 1.2s

# Down to "Preview style" and cycle the render profiles
Down
Sleep 400ms
Down
Sleep 600ms
Right
Sleep 1.4s
Right
Sleep 1.4s
Right
Sleep 1.4s
Right
Sleep 1.2s

Escape
Sleep 1s

Hide
Type ":q!"
Enter
