# mdedit main demo: live view (edit inside the rendered document), then split view.
# Records the local release build with default settings:
#   cargo build --release && vhs demo/demo.tape
Output demo/mdedit-demo.gif
Output demo/mdedit-demo.mp4

Set Shell "bash"
Set FontSize 15
Set Width 1200
Set Height 720
Set Padding 12
Set TypingSpeed 45ms

# Fresh HOME so the recording uses default settings, not a saved config
Env HOME "/tmp/mdedit-demo"
Env COLORTERM "truecolor"

Hide
Type 'mkdir -p /tmp/mdedit-demo && cp demo/sample.md /tmp/mdedit-demo/notes.md && export PATH="$PWD/target/release:$PATH" && cd /tmp/mdedit-demo && clear'
Enter
Sleep 500ms
Show

Type "mdedit notes.md"
Sleep 400ms
Enter
Sleep 2.5s

# Move through the document: the block under the cursor opens as raw markdown
Type "jj"
Sleep 1.3s
Type "jjjjj"
Sleep 1.3s
Type "jjjjjj"
Sleep 1.6s

# Edit a task in place
Type "jjjjjjjjjj"
Sleep 900ms
Type "A"
Type " (with reminders)"
Sleep 600ms
Escape
Sleep 500ms

# Move into the table: the task renders again, the table opens
Type "jjjjj"
Sleep 1.4s

# Add a row
Type "o"
Type "| Faro | 1 | Beach day |"
Sleep 500ms
Escape
Sleep 500ms

# Back to the top: everything is rendered, including the new row
Type "gg"
Sleep 2.2s

# Split view: raw editor and rendered preview side by side
Ctrl+p
Sleep 1.4s
Type "jjjjjjjjjjjj"
Sleep 2s

# Save and quit
Type ":wq"
Sleep 500ms
Enter
Sleep 600ms
