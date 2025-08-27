# This file is intended to be used with https://github.com/charmbracelet/vhs
Output demo.gif

Set FontSize 12
Set Height 720
Set Width 1280
Set TypingSpeed 40ms

# WARNING: for sixel images to render in the recording you need to use the
# https://github.com/lusingander/vhs fork.
# See https://github.com/charmbracelet/vhs/issues/43#issuecomment-2342408251
# for the upstream discussion.
Env LOSRS__OUTPUT__FORMAT "sixel"
Env LOSRS__OUTPUT__PPI "96"
Env LOSRS__OUTPUT__BASE_FONT_SIZE "12"
Env LOSRS__OUTPUT__LINE_HEIGHT_SCALING "1.0"


Type "echo 'Here are some freshly created SRS cards'"
Sleep 0.5s
Enter

Type "cat pages/lens.md"
Sleep 0.5s
Enter

Type "losrs metadata pages/lens.md"
Sleep 0.5s
Enter


Sleep 3.5s
Type "clear"
Sleep 0.5s
Enter


Type `echo 'Let us review them. We will respond with "1 - Little effort" for the first and "4 - Did not recall" for the second'`
Sleep 0.5s
Enter

Type `echo 'Time of review and random seed are fixed for demo reproducibility, you do not normally need to specify them'`
Sleep 0.5s
Enter


Sleep 3.5s
Type "clear"
Sleep 0.5s
Enter


Type "losrs review pages/lens.md --at=2025-08-27T00:00:00Z --seed=1"
Sleep 0.5s
Enter
Sleep 3.0s
Space
Sleep 3.0s
Type "1"
Sleep 3.0s
Space
Sleep 3.0s
Type "4"


Sleep 3.5s
Type "clear"
Sleep 0.5s
Enter


Type "losrs metadata pages/lens.md"
Sleep 0.5s
Enter

Type "losrs metadata pages/lens.md | grep next.sched"
Sleep 0.5s
Enter

Type "echo 'The card we could not recall is due next day'"
Sleep 0.5s
Enter

Type "echo 'The card that took little effort to recall is due in two weeks'"
Sleep 0.5s
Enter

Sleep 3.5s
Type "clear"
Sleep 0.5s
Enter
