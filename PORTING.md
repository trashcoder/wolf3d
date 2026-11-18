# Rust port status

This project is a feasibility study. Public release or distribution is not
planned.

## Fortschrittsschätzung

Die Prozentzahl ist eine grobe, sichtbare Umsetzungsschätzung, keine gemessene
Testabdeckung. Für eine konstante Berechnung gelten diese Gewichte: Rust-Port
der Spiellogik 45 % (derzeit 99,9 % umgesetzt), SDL2-Laufzeit und Integration
20 % (99,9 %), Datensuche, Spielstände und Einstellungen 10 % (99,9 %), Paketierung
und CI-Konfiguration 15 % (87 %) sowie finale Builds und Laufzeitprüfung auf
Linux, Windows und macOS 10 % (0 %). Daraus ergeben sich gerundet 88,0 %.
Implementierungsanteile zählen erst als plattformübergreifend verifiziert,
wenn der fertige Build und der Start auf dem jeweiligen Zielsystem geprüft
wurden. Diese Gewichtung ersetzt frühere, zu optimistische Schätzungen; die
Änderung der Prozentzahl bedeutet keine Code-Rücknahme. Der Release-Workflow
startet ausschließlich bei einem Versions-Tag (`v*`), damit normale Änderungen
keine Zwischenbuilds erzeugen.

This repository contains the historical DOS/C release. The Rust application is
being migrated alongside it so the original material remains available for
reference. Rust now has an SDL2 window/input loop, a 320x200 software
framebuffer, 64-bit compile-time size/alignment checks for its SDL2 event,
audio-spec, and rectangle FFI layouts, a WL6/WL3/WL1 data-set finder, a
Carmack/RLEW map decoder with
checked MAPHEAD index arithmetic and slice ranges, a VSWAP wall-page reader,
VGA Huffman decoder, VGA picture-plane conversion matching DOS
`VL_MemToScreen`, extracted 256-color VGA palette,
original ceiling/floor color tables, player-start extraction, initial
`ScanInfoPlane` enemy/static spawn rules, key/health/ammo/extra-life pickups,
sliding doors that reject close requests until fully open, two-tile pushwalls
with moving collision/raycast geometry
and preserved connected-area tiles,
static sprites with original blocking-prop collision behavior including the
stove and weapon rack, DOS `DrawScaleds` visibility for dead guards using
their visible tile or a visible walkable neighbor, enemy-safe door closing,
enemy stand/walk/pain/death sprites, common enemy attack/death
animations with source-timed state-action shots, chase thinking on the same
tic that attack or pain expires, retained routes across those states,
source-timed human/dog walk cycles and ghost walk cycles, immediate
ghost pursuit from spawn, patrol path-thinking and animation on original state
timings and original one-tile-ahead patrol spawn positions, source-timed
randomized first-reaction delays and alert sounds,
line-of-sight detection using the DOS `CheckLine` grid crossings and door
intercepts, plus a continuous intersection against the port's moving pushwall,
with original area connectivity, close-range, and facing
checks, plus gunshot-noise alerting over connected areas,
with doors linking the areas on their passage axis from the first opening
movement until fully closed,
tile-based chase direction
selection, line-of-sight dodge selection with the original one-time first-attack
turnaround, full-speed diagonal enemy movement, source-timed chase think/move
states with DOS `DoActor` state-expiry timing at chase transitions and thinker
phase boundaries, retained chase and patrol routes until the destination tile
center even when movement is temporarily blocked, zero-seed
patrol timing, source-fixed boss facing (including DOS's initial north/south
cardinal directions for the five episode bosses) and ambush
behavior, 5,000-point awards for both Mecha-Hitler `KillActor` transitions,
successful knife-hit noise alerts, close-range
retreat direction for Schabbs, Gift, and Fat after reaching the selected
route's destination tile center, as in the DOS movement loop;
their first no-direction choice follows the DOS chase/dodge rule, and
original enemy-specific door
behavior: fully open doors are traversable for all. Other door states block
diagonal routes, dogs, and Fake Hitler. Enemies that can open doors select
a cardinal route and wait for it to open fully,
and Mecha-Hitler's original-position footsteps. When Mecha-Hitler's morph
starts with a zero-tic countdown, Hitler now resumes his six-tic chase cycle
after his first attack, matching the DOS `NewState` timer reset. Difficulty-
scaled enemy attacks and health use the original shared 256-byte
random table seeded from the DOS-style hundredths clock, including the separate
Schabbs/Gift/Fat/Fake-Hitler attack rolls,
with integer screen-space hitscan aiming tolerance and camera-relative
projection depth for target ordering and knife range, DOS `FL_VISABLE` actor-tile
visibility for shot accuracy, DOS summed-thrust run classification,
and player-connected-area plus line-of-sight gating for hitscan
damage and sounds, Fake Hitler dodge direction
selection, DOS-ordered randomized chase fallback including diagonal detours,
the DOS axis-based movement distance used for close-range chase attacks,
hitscan accuracy and damage measured from the actor's DOS-reserved destination
tile as updated by `TryWalk`, attack range measured from the reserved actor tile,
and the negative-distance
close-range attack chance while waiting for a door; visible firearm targets
have no artificial range limit,
player hitscan damage bands and miss chance also use the target's reserved
destination tile while it is still moving. Pushwall destination checks use each
living enemy's DOS-reserved `actorat` tile, including ghosts and enemies that
have not yet crossed into their next tile. While a pushwall moves, its full
current and next `actorat` cells now block player, enemy, and projectile
movement; the continuous wall shape still supplies ray intersection and
sub-tile collision checks. During the animation the ray caster also ignores
the stale solid map tile at the pushwall's source and draws the continuously
shifted box, matching the sub-tile `pwallpos` texture offset in `WL_DRAW.C`.
Movement-triggered ghost contact
damage uses the original area check, distance/tick-scaled chase attack rolls,
and source enemy alert,
attack, and death effects,
score-threshold extra lives and the original nine-life cap, with one
extra-life cue for every crossed score threshold even at the cap,
including doubled damage for enemies hit before their delayed first attack,
and the rare Map 9 death-scream substitution,
knife/pistol/machine-gun/chaingun
selection and firing with the original six-tic weapon animation cadence and
post-input timer start, DOS weapon selection lockout at zero ammunition,
attack-frame-gated ammo-pickup weapon restoration, trigger-phase ammunition use,
and attack actions that follow the currently equipped weapon after a pickup.
The chaingun repeat decision precedes consumption of its last round;
automatic-weapon repeat behavior, and weapon
fire sounds before target damage sounds as in `GunAttack`/`KnifeAttack`.
hand sprites, basic kill scoring and enemy drops, a decoded VGA bitmap
font with the original VGA status bar, weapon, key, face, and numeric pictures.
The live status bar redraws the DOS level, score, lives, health, ammunition,
keys, face, and weapon fields at their original character-cell and pixel
coordinates; the level number uses the original 8×8 status digits; the separate
16×16 digits are reserved for intermission screens. An independent comparison
against `StatusDrawPic`, `DrawPlayScreen`, and the `Draw*` HUD routines in
`WL_AGENT.C`/`WL_GAME.C` found no missing field in the current gameplay draw
path.
The HUD face cycles through its source frames and shows the original special
death picture when Schabbs's needle projectile delivers the fatal hit.
Collecting the chaingun shows the original pickup portrait and resets its
animation timer. Treasure pickup cues are queued before score-triggered
extra-life cues. The elevator uses the original level completion sound and
lets an accepted digital or synthesized cue finish before the fade.
Player damage and accepted pickups use the original timed red and white VGA
palette shifts with six-bit DAC arithmetic. Both counters advance after
actor actions, with damage taking visual priority. A pickup starts its
visible bonus shift on the next tic, as in the DOS play loop.
PC-speaker plus
digitized playback share SDL audio. PC-speaker tones keep their phase across
repeated bytes and restart on a new divisor, with integer sample accounting
at 140 Hz.
Mapped samples and synthesized effects have separate replacement priorities.
Before each normal campaign level, the Rust port shows the original
`GETPSYCHEDPIC` preload screen, status bar, progress bar, and fade sequence;
the picture is chunk 134 in WL6/WL3 and 138 in WL1. Its coordinates and the
70-tic input wait follow `PreloadGraphics` in `WL_INTER.C`; the horizontal
position converts DOS's 8-pixel character cell to framebuffer pixels. The
progress bar now fills during the visible wait instead of appearing complete
from its first frame; because Rust has already loaded level resources before
this display, this is a presentation animation rather than a measure of file
loading. Death restarts continue directly to the level, as in `WL_GAME.C`. The ray-cast
renderer reads map geometry and wall pages. Door auto-close uses the DOS
300-tic timeout, resets it when an enemy requests an already open door,
retries blocked closes each tick, and plays its cue when
closing begins. Each simulation tick advances doors, then pushwalls, before
updating the player and enemies, as in the DOS play loop. Player-triggered door
and pushwall movement therefore starts on the following tick. Boss projectiles
remain in flight until a wall collision or player hit, matching the looping DOS
states; save validation therefore does not impose a projectile age limit. Blocking
props and doors that are closed, opening, or closing stop projectiles. Fully
open doors let them pass. Rockets play the
original wall-impact sound, start the three-frame explosion at the collision
position with the remaining rocket-state tics, and leave
the original initial smoke puff followed by a three-tic four-frame trail.
Each puff consumes its first animation tic in the update that spawns it.
Fake Hitler's fire moves on its first tic and every six tics thereafter.
Needles move each tic, with the same six-tic sprite cadence as fire after
the first-tic frame advance. A targeted DOS-to-Rust state-table comparison
covered guard patrol and chase cycles, standard enemy and boss attack sequences,
their sprite frames and death sequences; it found no concrete WL6 mismatch in
that scope. Source comparisons also confirmed the base-game `T_Chase`,
`T_DogChase`, and `T_Ghosts` paths are present and found no concrete missing
WL6 action in those paths. The Spear-only `T_Will` path is outside this
base-game audit. A separate audit of `DamageActor`/`KillActor`, pain and death
states, score and drops, plus the checked pain/chase and boss-death transitions
found those behaviors represented in the Rust port. A route audit compared
`TryWalk`, `SelectDodgeDir`, `SelectChaseDir`, `SelectRunDir`, the chase/dog
thinkers, and door waiting, including diagonal door blocking and randomized
fallbacks; it found no concrete WL6 divergence in that scope. Any further AI
differences still need to be identified and verified against the DOS source.
An additional `ScanInfoPlane`/`SpawnStatic` comparison covered player and
static spawn handling, pickup tile mappings, difficulty fallthrough, patrol
starts and directions, dead guards, pushwall secrets, and ambush markers; it
found no concrete WL6/WL1/WL3 mismatch in that scope.
The `SetupGameLevel` comparison also verified the original 64×64 plane
handling, wall/floor initialization, door scan, row-major actor setup, ambush
replacement priority, counter baselines, demo random-table start, and death
restart reset. No concrete WL6/WL1/WL3 difference was found; smaller custom
maps remain an intentional Rust extension.
Meanwhile, chase, dodge,
run, and patrol routes now reject tiles reserved by
another active enemy and clear blocked route choices for retry. Dead ends
restore the DOS `nodir` state, so later chase or dodge attempts follow the
original turnaround and fallback rules. Player
separation follows DOS `MINACTORDIST` for the complete attempted move and only
applies within an area connected to the player; route selection does not treat
the player as an actor occupying a map tile.
Wall and blocking-prop collisions use DOS `TryMove`'s axis-aligned `PLAYERSIZE`
tile-overlap check, including its stricter diagonal corner blocking.
Enemy routing reserves each actor's next tile as soon as its route is chosen,
then refreshes those claims in actor order during the tick, matching DOS
`TryWalk`/`DoActor` tile occupancy instead of a position-distance block.
Chase sprite cycles advance on their DOS state tics even when a route is
blocked and no movement occurs.
Dogs use the DOS dodge-direction selector even when walls break line of sight,
and must select an open route before starting their jump attack; a fully blocked
dog cannot bite from place.
Dogs double their DOS `SPDDOG` speed on first sighting as in `FirstSighting`;
ghosts retain `SPDDOG` during pursuit.
Enemy hit chance now uses the prior drawn visibility state (`FL_VISABLE`) and
refreshes it after actor updates, preserving the DOS `DoActor`/`DrawScaleds`
frame boundary.
Patrolling enemies keep moving during their source reaction delay, which only
counts down on patrol thinker tics.
faithful AdLib music. Startup now has main-menu actions for new game, loading a
named save slot, high scores, Sound settings, Display settings, configurable
keyboard controls, original help articles on WL6/WL3 data, replaying the
attract/demo cycle, and quitting. F1–F10
remain reserved when rebinding keys. During gameplay, F1 opens those same
articles in the active SDL window; the existing ten-tic transitions fade into
and out of help while simulation and audio stay paused. The original VGA article
frame and timed text/picture reveals are retained.
Normal startup now shows the original 320×200 signon image from
`WOLFSRC/OBJ/SIGNON.OBJ`, followed by the DOS “Press a key” acknowledgement and
“Working...” message before the PG-13 screen. The build script extracts the
image bytes from the OMF object into Cargo's generated output; direct map,
save, and demo launches continue to skip the acknowledgement.
The in-game pause menu can start a new campaign through the original episode,
confirmation, and difficulty sequence. Cancelling episode selection returns to
the pause menu and keeps the current level; cancelling difficulty selection
returns to episode selection. A confirmed new game resets the campaign and
loads its selected starting map and difficulty. Pause, load-confirmation, and
help overlays retain the live 40-pixel status strip, as the DOS control panel does.
All nine pause-menu choices now fit above that strip in the 160-pixel play area;
mouse hit-testing uses the same compressed row spacing.
Attract timeout plays the four embedded DOS demos in a loop on WL6/WL3 data.
WL1 checks for the external DOS `DEMO0.`–`DEMO3.` files beside the game data
and plays them when present; if none is available, it proceeds to the main menu.
The main-menu `Back to Demo` action reuses this same sequence and preserves
the next demo index when returning from the menu.
Choosing Quit or pressing Escape from the startup menu now presents a quit
confirmation using one of the original DOS prompt strings; No is selected by
default and cancelling returns to the startup menu.
Demo playback preserves signed
recorded axis values at their full magnitude, as in DOS `PollControls`.
Each demo level initializes the original random-table cursor to zero before
spawning actors, as DOS `SetupGameLevel` does for playback.
Demo levels skip `PreloadGraphics`, matching DOS `PlayDemo`, which sets up and
reveals the level directly without the normal 70-tic preload wait.
Patrols and ghosts use their randomized initial state countdown. A zero
countdown keeps the initial sprite while that state's thinker continues to run.
Patrols spawn active. Other enemies run their states in player-connected
areas or after viewport rays reveal their reserved actor tile or a walkable
neighbor. Activation and cached `FL_VISABLE` visibility both use that DOS
`tilex`/`tiley` destination tile while projection continues to use the actor's
current position, matching `TryWalk` and `DrawScaleds`.
The moving pushwall's DOS-reserved map cell now blocks ray visibility for
actor activation throughout both movement tiles, including the second tile
whose static map plane is floor.
The main menu now uses the original
DOS options picture and selection cursor at the DOS text and cursor coordinates.
Episode and difficulty selection also show the original cursor sprite at the
positions derived from their DOS menu item geometry. Startup choices accept
letter shortcuts using DOS `LastASCII` search order, starting after the current
selection and wrapping through the list. A right-click cancels normal choice
menus and plays the DOS Escape sound; mouse-capture screens still use right-click
as a bindable button. A demo can also be started directly
with `--play-demo 0` through `--play-demo 3` on WL6/WL3 data or from matching
external `DEMO0.`–`DEMO3.` files on WL1. Positioned samples
convert the DOS attenuation values through the SB Pro three-bit voice register
curve, using the documented CT1345-compatible level steps; physical output can
still vary by sound card. The Sound screen can select an
available SDL audio output device, toggle digitized samples separately from
PC-speaker and AdLib fallback effects, and mute music or all effects. The
AdLib fallback reads the original register instrument and 140 Hz pitch data
from the separate archive chunk group and synthesizes it through the OPL2
model. The DOS source has its `SDL_ALSoundService` disabled with `#if 0`; this
port implements the service implied by its chunk format and timer call sites.
Music and AdLib effects now share one OPL2 register state in the SDL mixer.
Each output sample services effect pitch changes before IMF music events, so
music writes can overwrite the effect's channel-zero registers as on the DOS
driver. The effect instrument is installed only when its service starts, so a
muted or lower-priority effect does not modify the live music voice. The mixer
uses channel masking to preserve the separate music and effects mute controls.
The SDL layer retains the DOS waveform-select register initialization. The
custom YM3812 model honors waveform-selection enable, note-select, the YM3812
operator envelopes, rhythm voices, modulation, and chip-rate sampling. The
`oplon` experiment has been removed and the previous synth restored. The
increased output level is retained as a threefold gain over the previous
custom-synth output scale: the mixed operator sum now uses `12,000 / 3` instead
of `12,000 / 9`. SDL clips the result at its 16-bit output boundary. Source
inspection confirms the gain factor; subjective listening still needs the
user's playback check.
The first AdLib effect pitch now waits for one 140 Hz service interval after
the instrument is installed, matching the order in `SDL_ALPlaySound` and
`SDL_ALSoundService` rather than sounding on the first SDL callback.
The DOS sound-playing status and effect priority end as soon as the last
AdLib pitch byte is serviced; the Rust mixer now releases those controls at
that point while letting the OPL envelope tail continue to render.
Muting music stops the IMF sequencer and keys off the channels used by that
track, while preserving channel zero when an AdLib effect is active. Replacing
a track performs the same stop before starting the new sequence; enabling music
restarts the selected track from its first event.
The startup and pause Sound panels use the original FX, digitized, and music
title pictures, checkbox art, mouse-background picture, and DOS row positions.
The startup panel keeps the audio-output-device picker and Back action alongside
the persistent sound switches. Both startup and pause Sound panels present the
DOS `None` and `AdLib/Sound Blaster` choices as separate selectable options.
The saved music setting marks the active choice; selecting AdLib/Sound Blaster
restarts menu music after it was disabled. Mouse selection targets the
corresponding half of the music row.
The supplied WL1 graphics archive contains 144 picture entries, while the
source `GFXV_WL1.EQU` table describes 136. Its extra pictures shift the first
cursor and subsequent menu artwork by one chunk, then shift the status-bar and
later pictures by eight total. The VGA reader detects this archive layout and
maps source picture IDs to the matching archive chunks. This keeps the fixed
`C_OPTIONSPIC` in place, draws `C_CURSOR1PIC` instead of the inserted OPTIONS
title, and restores the intended `C_MOUSELBACKPIC` at the bottom of the menu.
The mapping was checked against the decoded picture dimensions and DOS
`DrawMainMenu`/Sound-menu draw calls. The startup cursor now uses the DOS
`MainItems.x` position (76 pixels); runtime verification is still pending.
The startup Controls page opens the original `C_CONTROLPIC` panel with its
checkboxes and six DOS-spaced rows. Its Customize Controls entry opens
`C_CUSTOMIZEPIC` with the mouse-background picture and DOS title stripe, then
shows the persisted Rust bindings in a compact list. That editor can rebind
the twelve gameplay actions, capture mouse and controller buttons for the
DOS-supported action maps, and rebind the separate DOS `dirscan`
North/East/South/West keys. East and West
turn normally and strafe while the strafe modifier is held. The defaults use
WASD for direct actions and arrow keys for `dirscan`; legacy default settings
are migrated to avoid assigning the arrows to both groups. All mappings
persist with the other settings, and Restore Defaults resets each group.
Mouse and controller input can be toggled independently and those
choices persist; connected controllers remain dynamically discoverable. Both
input types default to enabled to preserve the Rust port's existing automatic
SDL input behavior, and can be disabled separately. The mouse switch also
disables pointer navigation in menus and acknowledgement screens, and the
controller switch suppresses gamepad menu actions. Keyboard input remains
available. This follows DOS `Mouse(4)` and `ReadAnyControl` device gating.
Menu arrow navigation releases its repeat delay on key-up and now uses Rust's
own eight-tic repeat timer (about 114 ms), independent of each operating
system's keyboard-repeat delay. The mouse-sensitivity gauge keeps its separate
DOS ten-tic cadence.
The startup Controls menu includes the original mouse-sensitivity gauge with
the DOS ten-tic arrow-repeat cadence and movement cue; Enter commits and Escape
keeps the prior value. Sensitivity persists across runs, supports all ten DOS
levels, and applies the original
`10/(13-mouseadjustment)` horizontal and `20/(13-mouseadjustment)` vertical
control scales. Display settings persist fullscreen and window size presets of
640×480, 960×720, and 1280×960; the game window remains resizable. The play
viewport supports the DOS view-size range (4–19), persists the selection, and
scales wall, sprite, and weapon projection with the viewport. View size can be
changed from startup and pause display menus. During play, the pause display
opens a live world preview with arrow-key and mouse-motion adjustment, Enter or
left-click to save, and Escape or right-click to restore the previous size.
Held arrow keys and controller D-pad directions repeat every ten DOS tics and
play `HITWALLSND` on each step, matching `CP_ChangeView` in `WL_MENU.C`.
Mouse adjustment is gated by the in-game mouse setting. Its bottom instruction
panel matches the DOS `CP_ChangeView` layout and color bar while naming the
available mouse actions. Unlike DOS, the live preview immediately reprojects
the full scene while the view size changes; DOS only draws the prospective view
border until Enter. This remains an intentional preview behavior. The
startup selector remains a simple list. The
4:3 VGA picture scales to the renderer output and remains letterboxed when
the available area is wider. SDL windows request high-DPI output, and pointer
coordinates map between window units and renderer pixels. The Rust entry point
calls `SDL_SetMainReady` before SDL initialization so Windows and macOS do not
depend on SDL's C `SDL_main` wrapper. Fullscreen mode and independent music/effect
toggles persist in the platform user-configuration directory. DOS-localized gameplay
events for enemies, projectiles, doors, and BJ use source positions; sounds
that the DOS code played through `SD_PlaySound` remain global. Positioned effects
recalculate the original camera-relative position and facing throughout
digital sample playback, matching the DOS Sound Blaster Pro behavior. A full
call-site audit against the base-game DOS sources checked every world-space
`PlaySoundLocActor`/`PlaySoundLocTile` cue against the Rust actor, projectile,
and door events; it found no missing or incorrectly centered gameplay cue.
Synthesized PC-speaker and AdLib effects are centered. Gretel's hitscan uses
the original standard Nazi firing cue. Selection menus fade in and out
over ten DOS tics, and can still be closed immediately from the window. Keyboard
and mouse row changes use the DOS half-step and cursor sounds, with the cursor
cue delayed by eight tics; ordinary menu acceptance and cancellation use
`SHOOTSND` and `ESCPRESSEDSND`. The Sound panel follows its own DOS callback
rules for individual mode changes. The selected cursor alternates between `C_CURSOR1PIC` and
`C_CURSOR2PIC` on the DOS 9/9/71-tic cycle after fade-in in the startup menu
and the standalone episode and difficulty selectors.
Mouse hover and left click navigate and select the startup, episode, difficulty,
settings, and jukebox lists; pointer coordinates account for fullscreen
letterboxing and high-DPI renderer scaling.
The Display Settings persist a choice of `Classic`, `HQx 4x`, `xBRZ 4x`, or
`CRT Shader`, available from startup and in-game. A shared output path enlarges
the 320×200 framebuffer to 1280×800 before SDL presents it, preserving the
4:3 display viewport and widescreen side bars. HQx and xBRZ use their pixel-art
edge reconstruction algorithms; Classic uses nearest-neighbor replication.
The CRT effect applies scanline attenuation and an RGB phosphor mask in a
software shader pass so it works with the existing SDL backends on all targets.
These are display filters and do not increase the renderer's internal scene
detail. The earlier linear `Soft` option was removed because it visibly blurred
pixel edges; settings saved with it fall back to `Classic`.
The HQx dependency is LGPL-2.1-or-later; the Rust xBRZ port is GPL-3.0-only.
These filters are part of the local feasibility study and are not included in
any release or distribution package. The custom CRT shader calculations run in
the software presentation path to keep behavior consistent across SDL targets.
The in-game control panel and its sound, control, display, and save/load views use
ten-tic fades for menu-to-menu changes. Entering the in-game control panel
fades out over 30 DOS tics and its selected view fades in over ten. Returning
to gameplay fades the active view out over ten tics and reveals gameplay over
30; closing the F1 article uses 30 tics in both directions. Simulation and
audio remain paused throughout these transitions. The nine-item pause root
menu moves exactly one entry up or down and wraps at the list ends, matching
`HandleMenu`'s active-item traversal in `WL_MENU.C`.
Gameplay accepts progressive SDL GameController left-stick movement and turning
with a fixed 8,000-unit dead zone, plus A to fire, B to use, X to run, left
shoulder to strafe, and the D-pad to select weapons. The progressive stick is
an SDL adaptation of DOS's optional `joystickprogressive` mode, using normalized
axis values and a fixed dead zone. In the in-game menus, the D-pad navigates, A
accepts, B goes back, and Start pauses or resumes. Startup, episode, difficulty,
settings, and jukebox menus also accept D-pad navigation, A to select, and B or
Start to cancel.
Compatible controllers can be connected or removed while playing. Relative
mouse movement and the DOS default mouse buttons also work: left fires, middle
strafes, and right uses. Horizontal and vertical motion use the persisted DOS
mouse sensitivity, defaulting to `mouseadjustment=5`; focus loss clears
held buttons and relative motion is disabled outside active gameplay.
Horizontal mouse movement now turns in the DOS direction after translating
DOS's north-positive angle to Rust's south-positive screen angle.
The SDL Pause key now shows the original `PAUSEDPIC` at the DOS screen
coordinates, pauses audio and simulation, waits for a fresh key or mouse
press, and clears held input before gameplay resumes.
Enemy attack, death, and alert sounds retain their original local/global
behavior, including Hitler's global death action sound; local sounds and door
open/close cues use the original left/right attenuation maps with SDL stereo
mixing, and door cues play only when their area is connected to the player.
Schabbs, Gift, Fat, and Hitler now use the original death-camera waits,
message card, fizzle transitions, camera reposition, and repeated death sequence.
Their second death-state delay follows the DOS 140-tic digitized or 5-tic
non-digitized setting; Hitler's Slurpie cue follows the matching state boundary.
The message transition covers the area above the status bar with the DOS
70-frame pixel budget. The camera reveal uses the configured viewport and the
DOS 20-frame pixel budget. Simulation timing restarts after either transition.
Fizzle transitions now retain DOS's full 16-bit X value until clipping and
include the edge coordinates allowed by `FizzleFade`.
The level clock counts gameplay tics at 70 Hz, including boss death-camera
sequences as in DOS, and excludes menus, level reveals, and the player-death
sequence whose restart resets the level clock. Save slots still store elapsed
seconds, restored to the nearest tic. Results cap level time at 99 minutes.
Level entry, demo playback, and save restoration also reveal the viewport
with the DOS 20-frame budget. After death the reset level replaces the red
viewport through the same reveal. The reveal does not count toward level time;
keyboard actions are ignored during it, matching `CheckKeys` while `screenfaded`.
Mecha-Hitler's morph into Hitler retains the current route and direction, so
the new actor reaches the same destination tile center before choosing another
route, as in `A_HitlerMorph`. The spawned Hitler draws the DOS-randomized
six-tic chase countdown; a zero countdown keeps the initial thinker active,
and positive countdowns account for the morph tic's actor-list pass.
The Wolf3D intermission awards a fixed 15,000 points when entered for
floor nine or ten. The victory summary uses the first eight floors, with
unplayed floors contributing zero to each of the three averages. It now runs
before the episode ending article, matching DOS `Victory` then `EndText`. The
summary follows the DOS text positions and 160-line panel, and fades in and out
over 30 tics; its displayed total time uses the DOS 99:99 cap. Intermission and
victory preserve the last gameplay frame's 40-line status panel. Both screens
draw their labels and values with the original `L_*PIC` VGA glyph images. The victory
screen acknowledges keyboard, mouse, and controller input after fading in, as
DOS `IN_Ack` does. Walking onto the
original exit tile now runs
BJ's six-tile path-following celebration uses the DOS-randomized initial run
state timer and is first advanced in the same tic that reaches the exit tile,
matching `SpawnNewObj` and the actor-list pass. The player runs the DOS victory
spin and upward camera move while retaining the exit tile used by actor AI and
area connectivity. Door motion stops, while pushwalls, projectiles, timed
enemy-state actions, dog/ghost chase thinkers, and human standing/patrol
thinkers continue; ordinary human chase thinkers return immediately as
`T_Chase` does during victory. Hits still consume the DOS attack/projectile
random rolls and play source sounds, but `TakeDamage` is ignored by the
victory-flag guard. The last `thrustspeed` classification is retained for enemy
shots. The HUD face continues to animate only when the player entered victory in
the attacking state, matching `T_Attack`'s `UpdateFace` before its victory
guard; weapon attack timing stays frozen, and `DrawPlayerWeapon` hides the
weapon sprite. Use and fire input no longer triggers gameplay actions. BJ then
plays the three jump frames, yell, and final 300-tic pose before opening the
victory flow.

## Build policy

An explicitly requested Linux x86-64 preview compiled and opened shareware
map 1 with SDL2. Movement was checked using X11 input. This exposed and fixed
the shareware audio directory selection, VSWAP digital sample loading, and
the compact sprite column directory with signed post offsets. Game data
was supplied separately. Final cross-platform validation is still pending.

A second user-authorized Linux x86-64 release preview was built on 2026-11-16 as
`dist/linux-x86_64-preview/wolf3d-2026-11-16`. It sits beside the existing WL1
data files; no game data was recopied. The release build succeeded and its
`--help` path ran, but this dated binary has not been relaunched into gameplay.
It predates the later intermission glyph, mouse-turn, and door-texture corrections
and does not count as final Linux, Windows, or macOS validation.
A third user-authorized Linux x86-64 release preview was built on 2026-11-16
from the corrected tree as
`dist/linux-x86_64-preview/wolf3d-2026-11-16-2`. Its `--help` path ran, and
its SHA-256 is
`2c781fbc7abe662dd3edb1d21fa88d9d4c951f6815193354cfb6d33e6c99f287`.
The release build completed with 34 compiler warnings; gameplay was not
rechecked in this run, so this preview does not count as final platform
validation. The rough weighted migration estimate at build time was 86.7%.
An additional user-authorized Linux x86-64 preview was built on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17`, beside the existing WL1 data
files; no game data was recopied. The first build attempt exposed mismatched
integer widths in the YM3812 rhythm phase-bit code, which were corrected before
the successful release build. The resulting binary's `--help` path ran and its
SHA-256 is
`19071946b43fcfbe60387056abe03cb351fdcb9287c6c5b63d409ed1350d1e10`.
Compilation completed with 35 warnings. Gameplay was not launched in this
check, so the preview does not count as final Linux validation. No further
builds, tests, or packaging should run until the Rust engine migration is
functionally complete unless the user explicitly requests an intermediate
build; then generate the target builds and run the cross-platform validation
and packaging steps.
A further user-authorized Linux x86-64 preview with the VGA planar-image fix
was built on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-vga-fix`, beside the existing WL1
data files; no game data was copied. Its `--help` path ran, and its SHA-256 is
`af3a28db841309d05c090abe8e46b1c6a6a4a80ae4cd2fde8e363a79edaa5bcd`.
Compilation completed with 35 warnings. The menus and HUD have not yet been
visually rechecked in this build, so it does not count as final Linux
validation. Normal startup shows the original
PG-13 screen and title picture. The title, credits, and high-score attract
pages fade in and out over the DOS 30-tic intervals; their 15- and 10-second
input waits begin after fade-in. The title shows only the original `TITLEPIC`,
without an extra start prompt. The credits stay for 10 seconds and the
high-score table for 10 seconds. The timed high-score attract screen also
accepts keyboard, mouse, and controller
input. If those screens time out, it cycles through the four original demos
and returns to the title; keyboard, mouse, or controller input skips the
remaining attract screens and opens the main menu. The timed PG-13, title, and credits screens
accept the same input types as DOS `IN_UserInput`. Use the
up/down arrows and Enter to choose. New Game opens episode and difficulty
selection; Load Game lists the available named save slots. The Sound menu
persists separate music/effects switches, the PC-speaker/AdLib fallback mode,
and the selected SDL audio output device; turning music back on restarts the
original menu track. The Display
menu selects 640×480, 960×720, or 1280×960 window sizes.
Default controls are **W/S** or
up/down arrows to move, **Shift** to run, **A/D** or left/right arrows to turn,
and **Alt+A/D** or **Alt+left/right** to strafe,
**1–4** to select owned weapons, **E** to use a door or elevator, and **Ctrl**
to fire while held. Use the Controls screen to rebind these keyboard actions.
During gameplay, an SDL GameController uses the left stick for progressive
movement and turning with an 8,000-unit dead zone, A to fire, B to use, X to
run, left shoulder to strafe, and the D-pad to select a weapon. In the in-game
pause menus, use the D-pad, A to accept, B to go back, and Start to pause or
resume.
When a mouse is available, relative X turns, relative Y moves forward or back,
left fires, middle enables strafing, and right uses doors or elevators, using
the original default mouse sensitivity. Enemy hit chance uses the same
per-tic thrust sum as DOS, including movement contributed by the mouse.
The SDL Pause key opens the original brief pause card; press any key or mouse
button after releasing the Pause key to resume.
F2 opens the save slots, F3 opens load slots, and F4 opens sound settings. **F5**
opens the view-size panel, F6 opens control rebinding, and F7 asks to end the
game. **F8** quick-saves an occupied selected slot, or opens save-slot selection
when it is empty. F9 asks for confirmation before restoring the selected slot;
if it is empty, F9 opens the load-slot list. F10 confirms quitting the game.
F1-F10 are reserved while rebinding controls. Slot names can be edited in the
save/load slot view with F8; existing save names update in the slot file, while
names entered for empty slots apply on the next save. Loads still require
matching map, difficulty, WL1/WL3/WL6 variant, app version, and source map data.
Saving an occupied slot from the menu requires confirmation. Slots use the same
validated, backed-up save format. `--load-slot <1-10>` starts from
a save slot and selects its saved level and difficulty automatically.
Collect keys, health/ammo/life bonuses, machine-gun/chaingun pickups, and pushwalls;
press **Escape** to pause and choose **Resume** or **End Game**; ending the game requires confirmation and opens the high-score flow. The status bar displays health, ammunition, lives, score,
floor, keys, face, and selected weapon using original VGA pictures. The window title
also shows kills and discovered pushwalls. Losing all health hides the weapon
sprite and turns the view toward the attacker along the shorter arc at two
degrees per tic. It then fades the viewport to VGA red through the DOS pixel sequence, with
the status bar still visible. A key press or the 100-tic pause ends the red
screen before consuming a life
and restarting the current map; each attempt's time, kill, and treasure ratios reset.
When no lives remain,
the game-over state appears in both places.
Using an east/west-facing elevator flips its switch to the activated wall
texture and ends the current run; the alternate floor
tile selects the episode's secret map. Final-boss completion opens the
registered game's original ending article pages with picture commands and
keyboard navigation; shareware data falls back to a victory summary. Normal
map completion advances to the next map while
preserving health, lives, score, and weapons; completing a secret map returns to
its episode's configured elevator-back map. Completed levels show a partial
intermission summary with the original hero portrait, time/par comparison,
kill/secret/treasure ratios, and the corresponding original score bonuses.

An additional user-authorized Linux x86-64 preview containing the WL1 graphics
chunk remapping was built on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-menu-fix`, beside the existing
WL1 data files. Its `--help` path ran successfully; its SHA-256 is
`f83059d8132467f0fb3b1121278bc4ba40fc73ff2a7fd6d7d508976f158813a1`.
Compilation completed with 35 warnings. The game was not launched for a visual
check, so this intermediate build does not count as final Linux validation.
The estimated overall implementation status remains 88.0%; final Linux,
Windows, and macOS builds and runtime checks are still outstanding. The user
will handle runtime testing when those builds are ready.

An explicitly requested Windows x64 preview was built on 2026-11-17 as
`dist/windows-x64-preview/wolf3d-2026-11-17.exe` and packaged as
`dist/windows-x64-preview/wolf3d-windows-x64-2026-11-17.zip`. The archive
contains the executable, SDL2 runtime, license and README, and the WL1 data
files copied from the existing local shareware preview at the user's request.
The PE executable format and ZIP contents were checked; Wine could not run the
`--help` smoke check in this environment because its local server socket was
blocked. This cross-compiled intermediate preview has not been launched on
native Windows and does not count as final Windows validation. Its executable
SHA-256 is `3acc31f490dccef4ad8fcc5cd0e0b99e070bfd9a277963acee0ac999223d63a3`.
The rough weighted implementation estimate remains 88.0%.

At the user's explicit request, a Linux x86-64 release build with the
persisted swastika-symbol display toggle was created on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-symbol-toggle`, alongside the
existing local WL1 data files. The executable is 1.4 MiB and has SHA-256
`204ed85f94d3c30dfd6939fc4c3de3d1096d815f42a4168aa5a8bcbc7f5961c5`. It
compiled successfully with 44 existing compiler warnings. No runtime launch or
gameplay check was run, so it does not count toward final Linux validation;
the rough weighted implementation estimate remains 88.0%.

After the WL1 wall-texture scan found the small armband emblem on portrait
pages 6/7 and 20/21, an explicitly requested Linux x86-64 release build with
the targeted armband-symbol masking was created on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-armband-symbol`, alongside the
existing local WL1 data. It is 1.4 MiB with SHA-256
`0b8008dbdc2ef1d8d950d4f4c8d0652a1b22aa9c219a88f10a3e6b1c3d24f569`. The
release build succeeded with 44 compiler warnings. No gameplay/runtime check
or tests were run; it is an intermediate preview and does not count toward
final Linux validation. The overall implementation estimate remains 88.0%.

At the user's explicit request, a Linux x86-64 preview with faster held-key
menu navigation was built on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-keyboard`, beside the existing
WL1 data files. The release build succeeded with 44 compiler warnings, and its
`--help` path ran successfully. Its SHA-256 is
`c8f468e0370693171eb97c7a0fa7c8af01322de8a278c5a2b5dc24b03a33bc94`. Gameplay
was not launched, so this preview does not count as final Linux validation;
the overall implementation estimate remains 88.0%. This binary predates the
later OS-independent keyboard-repeat source change and does not include it.

At the user's explicit request, an experimental `oplon`-backed Linux x86-64
executable was built on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-oplon`, alongside the existing
WL1 game-data files. The release build succeeded with 50 compiler warnings;
the 1.4 MiB ELF executable has SHA-256
`aa8b58e273eb2757ccfa1f9c0a04d29cc84f7ed10801806289e7504ff5b2a8c6`. It was
not launched. This binary is retained only as a historical experiment and no
longer corresponds to the current source. The weighted estimate remains
88.0%.

After the user asked to remove `oplon` while keeping the volume increase, the
custom YM3812 implementation was restored and a Linux x86-64 release build was
created on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-opl-restored`. The build succeeded
with 49 compiler warnings; the executable is 1,396,744 bytes and has SHA-256
`5511f07bf05fdab33c56acd7257dff1ea051158d1417ad2cee2a9ec1fd62f374`. It is
alongside the existing WL1 data files. No tests or listening check were run;
source inspection confirms that the prior threefold volume increase is carried
over to the restored synth. The overall estimate remains 88.0%, and the user's
listening check is still needed to judge perceived loudness.

The two historical Linux previews show that the `oplon` experiment did not
cause a measured size increase: the immediately preceding `keyboard` executable is
1,394,440 bytes, while the `oplon` executable is 1,383,368 bytes. The latter
is 11,072 bytes smaller, though other source changes between those builds mean
this is not an isolated package benchmark. The `oplon` release archive is
134 KiB before link-time optimization; the final release already enables LTO,
one codegen unit, and stripping. The previously mentioned roughly 600 KiB
binary is not present in the current preview directory, so its comparison
point cannot be independently checked here. Further size tuning should be
measured against rendering performance and audio quality.

At the user's explicit request, a Linux x86-64 release preview was created
before implementing the image-filter setting on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-pre-resolution`. The 1,396,256-byte
executable has SHA-256
`751bafb575249be6217cefcd8b26b78f592bdd34183f47f59b8cd975508bc6fd`.
Compilation succeeded with 50 warnings and the `--help` path ran successfully;
no gameplay check or tests were run. This intermediate binary does not include
the resolution/filter change and does not count as final Linux validation.
The overall implementation estimate remains 88.0%.

After the user requested a Linux build with the `Classic/Soft` image filter, a
release executable was built on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-soft-filter`, beside the existing
local WL1 data files. The ELF x86-64 binary is 1,399,840 bytes with SHA-256
`df83cb36139a76bd096c980ce150cf30867ef67070867183c4247d1460364da3`.
`cargo build --release` succeeded with 50 warnings, and `--help` ran
successfully. No gameplay launch or test suite was run; the overall
implementation estimate remains 88.0%.

At the user's explicit request, the HQx/xBRZ/CRT-filter Linux x86-64 release
preview was built on 2026-11-17 as
`dist/linux-x86_64-preview/wolf3d-2026-11-17-hqx-xbrz-crt`, beside the existing
WL1 data files. The stripped ELF binary is 1,789,000 bytes with SHA-256
`03e74e5db3a5e16c4ac5a2e43195541ffd83ef6b527a602830149fa2969b0bb6`.
The build succeeded with 51 compiler warnings and its `--help` path ran
successfully. No gameplay launch or visual filter check was run, so the new
rendering modes still need the user's runtime review. The overall
implementation estimate remains 88.0%.

The SDL2 development library must be installed for each target. Linux release
packages build in an Ubuntu 22.04 container on the current GitHub runner to
retain its glibc 2.35 compatibility baseline; the release workflow checks the
executable's required glibc symbol version before packaging. Linux local builds
use the system SDL2 linker paths; on Windows and macOS, set `SDL2_LIB_DIR` when
the linker library is outside its default search path. macOS SDL2 framework
installations can use `SDL2_FRAMEWORK_DIR` to select framework linking. Windows
ZIP packages must include the matching `SDL2.dll`; macOS app bundles and Linux
AppImages must include SDL2's runtime library. Original game data is not bundled
by default.
The Windows release target statically links the Microsoft C runtime so the
executable does not require a separate Visual C++ runtime installation; SDL2
remains a bundled `SDL2.dll` dependency.

Packaging scripts are in `scripts/`. The Linux AppImage script expects
`linuxdeploy` and `appimagetool`; it collects the SDL2 shared library from the
release executable's runtime dependencies. The Windows PowerShell script
expects `SDL2_DLL_PATH` to name the matching `SDL2.dll`. For a local preview,
`-GameDataDir` copies one complete WL6, WL3, or WL1 data set into the package's
`data/` directory; the release workflow does not pass that option. The macOS DMG script
expects separate x86-64 and arm64 release executables plus
`SDL2_FRAMEWORK_BUNDLE`, a directory containing a universal `SDL2.framework`.
It rewrites the executable's SDL2 dependency to the copy inside the app bundle.
For public macOS distribution, `MACOS_CODESIGN_IDENTITY` can sign the app and
framework; `MACOS_NOTARY_PROFILE` additionally submits and staples the DMG
through Apple's notary service. The release workflow can import a Developer ID
certificate from the `MACOS_DEVELOPER_ID_P12_BASE64` and
`MACOS_DEVELOPER_ID_P12_PASSWORD` secrets, and create the `wolf3d-notary`
profile from `MACOS_NOTARY_API_KEY_BASE64`, `MACOS_NOTARY_KEY_ID`, and
`MACOS_NOTARY_ISSUER_ID`. With no certificate secrets the workflow still
packages an unsigned, unnotarized DMG and warns that Gatekeeper may require
manual approval; with a certificate but no notary secrets it signs the app and
warns that Gatekeeper may still object. A notarized DMG is stapled and
validated by the packaging script.
These scripts only package existing release files; they do not compile the
port. Original game data is copied only when a local Windows preview explicitly
provides `-GameDataDir`.

The normal launch path checks for the map, VGA graphics, swap-page, and audio
files for one complete WL6, WL3, or WL1 data set in `--data`, `WOLF3D_DATA`, a
`data/` directory beside the executable, the platform's usual user data
directory, the macOS app bundle's `Resources` directory, the current directory,
or its `data/` subdirectory. Data filenames are matched case-insensitively and
opened using their actual on-disk names. Full game data files are not included; the one
256-color palette table was extracted from the Borland object already present
in this source release.
Linux uses `XDG_DATA_HOME` (or `~/.local/share`) for game data, saves, and
high scores, and `XDG_CONFIG_HOME` (or `~/.config`) for settings. Existing
settings in the previous data-directory location remain readable; future
settings writes use the configuration directory. Windows uses absolute
`APPDATA`, then `LOCALAPPDATA`, then the user's roaming profile fallback;
macOS uses `~/Library/Application Support/Wolfenstein 3D`.

The startup sequence carries the original intro IMF track across the PG-13,
title, credits, and automatic high-score screens when SDL audio is available.
The manually opened high-score screen uses the original ROSTER track; episode
and difficulty selection use the original menu track, carrying the sequencer
state across those screens and the other implemented menus. Help articles now
fade in the first page over ten DOS tics and fade out over the DOS 30-step
`VW_FadeOut` sequence when dismissed with Escape, in both the standalone
article window and in-game F1 overlay. Returning from F1 then fades the game
back in over 30 DOS tics.
Level-complete intermissions play the original `ENDLEVEL_MUS` track. The
original jukebox track list can be opened with `--jukebox`; it plays available
IMF tracks and remains open while tracks are changed. The pause root now draws
the original `C_OPTIONSPIC`, mouse background, stripes, and main-menu window;
the keyboard-rebinding view draws the original `C_CUSTOMIZEPIC` title art.
Pause save/load views use the original load/save title pictures, mouse
background, striped header, dialog position, slot outlines, and cursor art.
Before file I/O, the loading/saving card is presented using the DOS window,
floppy picture, and label positions. The floppy picture alternates at the Rust
save/load I/O steps, following the DOS `DiskFlopAnim` updates where the file
format has corresponding operations. In-game save/load now keeps the card
visible for at least 350 ms and continues flipping the disk after I/O completes
when the file operation finishes too quickly. Loading a save from the startup
selector shows the selected slot and the loading card while the save is
deserialized. The gameplay status bar is redrawn each frame from current game
state; a DOS-source comparison found its bar, weapon, level, score, lives,
health, ammo, keys, and face fields and their coordinates present in both
WL6/WL3 and WL1 paths.
The 2026-11-17 preview screenshots exposed that decoded picture planes were
being treated as row-major pixels. `VgaArchive::picture` now rebuilds chunky
pixels from the DOS plane-major, row-within-plane layout. This corrects the
shared path used by menu artwork, episode/difficulty illustrations, status-bar
art, and HUD pictures. The conversion is source-checked against `ID_VH.C`'s
`VWB_DrawPic` and `ID_VL.C`'s `VL_MemToScreen`; a regenerated gameplay preview
is still needed for runtime confirmation.
The pause root now includes the DOS `VIEW SCORES` action. It displays the
original high-score artwork with the ROSTER track, waits for an input, then
fades back to the paused control panel and its menu music.
The startup Controls action now opens the DOS `C_CONTROLPIC` hardware panel
with its original artwork, checkbox art, cursor placement, and six-row layout.
Mouse enablement, controller enablement, mouse sensitivity, the extended
control editor, restoring defaults, and returning to the main menu are
available there. The DOS joystick enable switch maps to SDL GameController
input; joystick-port and gamepad-type selectors have no SDL equivalent. The
extended keyboard, mouse-button, controller-button, and movement bindings are
still presented in a Rust per-action editor, and the in-game pause control
panel retains its separate compact controls view.
The startup and in-game Display Settings now include a persisted
`Swastika Symbols` show/hide toggle, hidden by default for existing and new
settings files. The renderer masks only the central emblems in WL1 VSWAP wall
pages 4/5, 10/11, and 34/35, retaining the flags, medallion rings, wreaths,
eagle artwork, and surrounding wall texture. It also replaces only the dark
swastika ink in the Hitler portrait armbands on pages 6/7 and 20/21; the white
roundel, red armband, portrait, and frame remain. The toggle takes effect in
the current frame and can restore the original pixels immediately. The local
WL1 wall textures, VGA pictures, and sprite sheets were inspected. The user
does not have WL3/WL6 data, so equivalent graphics inventories for those sets
cannot be performed and are out of the current work scope.
The gameplay pause menu now packs all nine root entries into the area above the
preserved status bar; its mouse hit regions use the same tighter row spacing.
During a level, Escape opens a pause menu that freezes simulation, switches to
the DOS control-panel music after fading out, and restarts level music after
returning. It can resume, save or load any of ten named slots, open the four
persistent sound toggles, rebind controls, change fullscreen and window-size
settings, or leave the current level. The separate Pause key screen freezes
gameplay audio. Mouse hover and left click also select and activate items in the
pause root, save/load, sound, controls, display, and confirmation menus; right
click cancels. DOS shortcuts F1-F7 now open help, save, load, sound,
view-size, control, and end-game views. F4 returns directly to gameplay when
the Sound panel is closed; the same panel returns to the pause root when opened
from there. F6 now returns directly to gameplay when its Controls view closes;
Controls opened from the pause root still returns there. F7's end-game prompt
overlays the active view without fading or stopping music, accepts Y/N as in DOS, and returns directly to gameplay when
cancelled. F10's quit prompt overlays gameplay, selects from the original nine
messages using two DOS random-table draws, and keeps music running until a
decision; confirming it stops audio and fades out. F5's view-size screen
returns to gameplay after either accepting or cancelling; when opened from
Display Settings it returns there instead. F8
quick-saves the selected slot, or
opens save-slot selection when that slot is empty. F9 confirms and loads the
selected slot; F10 asks before quitting the game.
End-game from Escape continues through the red death pause. DOS F7's confirmed
quick end sets lives to zero and moves directly to the game-over transition.

The command line accepts `--map <index>` to start at a specific map and
`--difficulty baby|easy|medium|hard` to skip the interactive difficulty choice.
Without `--map`, New Game offers the episodes present in the data set; without
`--difficulty`, it shows the difficulty menu. `--help` prints the launch
options. `--play-demo <0-3>` plays an embedded DOS demo on WL6/WL3 data and
returns to the shell when it ends or a key is pressed. `--load-slot <1-10>`
loads a save independently of the current level. `--jukebox` opens the
standalone music selector and needs a supported game data set.
Campaign play advances through the data archive and preserves the
main inventory between levels. Secret exits use the original elevator/floor-tile
rule. The regular time and ratio bonuses apply to the first eight floors,
using the 48 par times from the DOS table. The time bonus tally starts at
zero and plays its cue at zero and every 50 counted seconds thereafter.

## Migration sequence

The intermission animates both original BJ breathing frames during tally
sounds and while waiting for input. Each screen starts on the first frame.
The intermission fades in before the tally and fades to black on exit.
Each fade uses the DOS 30-step budget timed at the port's 70 Hz rate.
The gameplay screen also fades to black on completion, victory, or game
over. Level exits clear the HUD keys for the final gameplay frame.
The intermission now counts the time bonus and completion ratios upward with
PC-speaker tally cues. Keyboard, mouse, and controller presses can skip the
count, and zero/full completion ratios wait 30 DOS tics before their bonus cue.
Its fixed and changing text now uses the original `L_*PIC` glyph images, with
DOS character widths and line spacing; the status score updates through the
original HUD number pictures during the tally. The current-time digits now
start at the exact DOS `WL_INTER.C` x-coordinate.
The end-level bonus is applied when the tally finishes or is skipped, matching
`GivePoints` after the DOS tally; the preserved status bar redraws its final
score and any extra life with the original latched-number pictures.
Level completion and victory preserve the original 40-pixel HUD strip before
the gameplay fade-out; DOS clears and redraws only the upper 160 pixels.
Ending articles read
their text, color, layout, bar, and picture commands, including timed picture
delays. Immediate `^G` pictures adjust text margins; timed `^T` pictures do
not, matching `WL_TEXT.C`. Content laid out before a `^T` remains visible while
that command waits, matching its `VW_UpdateScreen` before the delay; the timed
picture appears only when its delay expires. Input remains blocked through the
page delays and the first-page ten-tic fade. Escape fades articles out over the
DOS 30-step `VW_FadeOut` sequence. They use the DOS seven-pixel advance for
repeated spaces after words. The
high-score screen fades in and fades out over ten DOS tics. Episode and
difficulty selection now use the original menu background and illustration
pictures. Ten persistent named save slots are available, with
keyboard slot selection, name editing, overwrite confirmation, and the existing
map/data compatibility checks. A source audit of the base-game VGA screen and
transition paths found counterparts for the sign-on, attract, preload,
intermission, victory, and menu images checked; no additional missing screen
was confirmed in that scope. Remaining work includes closing further
source-confirmed base-game behavior gaps and preparing final Linux, Windows,
and macOS builds for the user's runtime testing. The custom OPL2 core is
restored after the `oplon` comparison; output gain is preserved, while the
user's listening check will confirm the resulting loudness and sound. WL3/WL6 art inventories
await access to those data sets. All screens now reuse one SDL window,
renderer, and framebuffer texture for the lifetime of the application instead
of creating and destroying video resources on each screen transition. The intermission no
longer draws an extra prompt over the retained status bar. The
title, credits, and automatic high-score attract screens now pass the original
intro track sequencer and live OPL2 synthesizer state through screen changes;
the tune therefore keeps playing after the first title screen instead of
resuming mid-song on an uninitialized synthesizer. The audio archive
now decodes the original IMF track chunks and maps campaign
levels to their source tracks. The victory summary now passes the active IMF
player into its ending article, so the original `URAHERO_MUS` track continues
instead of restarting. A simplified OPL2 register model and 700 Hz IMF
sequencer feed SDL during gameplay; the sequence loop boundary now waits for
the original next service tick before its first register write. The OPL2 model
honors YM3812 wave-select enable and its distinct waveform 3, and applies the
YM3812 key-scale attenuation slopes separately from operator total level.
SDL's level audio callback mixes music, digitized samples, and synthesized
effects concurrently. Each effect path has its own replacement priority, as
in the DOS Sound Blaster path. The weapon face cue checks the synthesized
effect path, matching DOS `SD_SoundPlaying`. The OPL2 model feeds its last
two modulator samples back into the modulator's phase and uses the previous
modulator sample in the carrier path. Both paths now quantize normalized
operator output to signed YM3812 units using its 4084 full-scale output
([Nuked-OPL3's exponent table](https://github.com/nukeykt/Nuked-OPL3/blob/master/opl3.c#L2346-L2350));
feedback uses the original arithmetic `2^(feedback-9)` shift from
[its `OPL3_SlotCalcFB`](https://github.com/nukeykt/Nuked-OPL3/blob/master/opl3.c#L3394-L3415).
The feedback history now advances at the chip sample rate. The OPL rhythm
register now triggers all five percussion voices. Hi-hat, snare, tom, and cymbal use
independent operators; hi-hat, snare, and cymbal follow the YM3812 phase
selection formulas. Percussion noise now uses the 23-bit right-shifting
register, feedback taps 14 and 0, and reads bit 0 before advancing, matching
[`OPL3_PhaseGenerate`](https://github.com/nukeykt/Nuked-OPL3/blob/master/opl3.c#L3126-L3294).
Bass-drum output follows its rhythm-mode carrier route and level. Operator
phase generators advance while their envelopes are off, as the YM3812 slot
processor does. Waveform amplitude now follows mathematically reconstructed
YM3812 log-sine and exponential tables, with the 4084 signed output scale and
the original phase sign rules. Each 0.1875 dB hardware attenuation unit maps
to eight finer exponential-ROM units, matching the reference `eg_out << 3`
scale ([ROM formulas and waveform functions](https://github.com/nukeykt/Nuked-OPL3/blob/master/opl3.c#L2272-L2350),
[`OPL3_EnvelopeCalcSin0`–`Sin3`](https://github.com/nukeykt/Nuked-OPL3/blob/master/opl3.c#L2500-L2639)).
Operator envelopes now use the YM3812's shared timer, 9-bit attenuation value,
discrete rate-step table, attack/decay/release rules, and 49,716 Hz chip-sample
clock. These changes replace the previous continuous envelope approximation.
Each chip sample uses the current attenuation before the shared timer advances
the envelope for the following chip sample. The transition rules follow
[`OPL3_EnvelopeCalc`](https://github.com/nukeykt/Nuked-OPL3/blob/master/opl3.c#L2822-L3105)
and the rate-step pattern follows
[`eg_incstep`](https://github.com/nukeykt/Nuked-OPL3/blob/master/opl3.c#L2447-L2457).
The operator phase accumulator now emits its current 10-bit value before
advancing once per 49,716 Hz chip sample, matching the YM3812 phase generator
and preserving phase zero on key-on. The chip-rate samples are averaged into
each SDL output sample. Noise, shared envelope clocks, amplitude modulation,
and vibrato all advance once per chip sample. Waveform lookup uses the YM3812's
10-bit phase output. Zero envelope rates hold their current level. Maximum
effective-rate attacks reach full output on key-on and enter decay on the
following EG step.
SL=15 uses the special 93 dB attenuation,
EG-TYP selects between percussive release and key-held sustain, and KSR adjusts
envelope rates using the selected F-number split. Operator AM and vibrato enable
bits and the global BD depth bits are also honored. AM uses the YM3812's
210-position triangle, 64-clock hold per position, and discrete attenuation
depth; vibrato follows the YM3812's zero-centered eight-step phase table and
upper-F-number scaling over an 8,192-step period. Intermission tally sounds
now use the same SDL callback mixer as IMF music and gameplay effects, keeping
one active audio device and the shared effect-priority rules. The Rust port now has the original
seven default high-score entries, DOS-compatible score/floor ordering using
episode-local floor numbers, migration of early Rust preview scores to those
floor numbers, name
sanitization, and a versioned JSON persistence format in the platform user-data
directory with backup recovery. The SDL high-score screen is connected to
victory and game-over, supports text entry for qualifying records, plays
ROSTER_MUS, and uses the original VGA high-score and column-label pictures
where the data set provides them.
The simulation
advances on a fixed 70 Hz clock with a 250 ms catch-up cap; forward, reverse,
run, and turn rates follow the original DOS control scales, with collision
substeps and the original player radius against walls, doors, and pushwalls;
combined keyboard, mouse, and controller input is clamped to the DOS
`PollControls` limit of ±100 movement units per tick. Recorded demo axes bypass
that clamp, matching the DOS playback path. A source comparison of
`PollControls` and `ControlMovement` against the Rust input combiner, axis clamp,
and thrust order found no additional base-game movement mismatch in that scope.
Sideways thrust precedes forward or backward thrust, with independent DOS
speed scales and clipping for each. Each thrust tries both axes together,
then X and Y individually in DOS order.
Turning applies whole degrees and retains the signed DOS control remainder
between tics. Saves carry this remainder, with zero used for older saves.
The blocked-movement cue plays when the synthesized sound channel is idle.
enemy collision uses DOS's one-tile per-axis `MINACTORDIST`, and dog bites use
the original two-tile per-axis threshold after subtracting `TILEGLOBAL`.
Player weapon attacks and enemy actions advance before bonus pickups, matching
DOS `DoActor`/`ThreeDRefresh` ordering and `GiveAmmo`'s attack-frame check.
Pickup collection uses the DOS `TransformTile` depth and lateral limits
and requires the tile to be reached by a viewport ray. The Rust death
sequence takes precedence over bonus collection after a lethal tick.
Static objects likewise draw only on tiles reached by a viewport ray.
Sprite projection uses the DOS minimum depth and separate offsets for
static objects and actors, with visible sprite edges clipped to the viewport.
Eight-way sprite rotation uses the offset from the active viewport center
and the DOS angle convention and integer divisions.
The four human enemy pain states apply the DOS two-way sprite rotation.
Boss projectiles launch at whole-degree angles. Rocket sprite selection
uses the flight angle directly, as in DOS `CalcRotate`.
Use commands target the tile faced before movement and run only outside
the attack state. Doors, elevators, and pushwalls share the original
`Cmd_Use` cardinal direction thresholds.
Door and elevator uses consume the held input until
release. A fresh fire press during an active attack starts a new attack
when the weapon returns to its idle state if the control is still held.
The in-game renderer now
uses the original default 240x120 viewport,
its DOS VGA border colors and one-pixel bevel, status-bar placement, and the original
FOCALLENGTH camera origin for walls and sprite projection. SDL scales the
320x200 VGA framebuffer to the intended 4:3 display aspect and letterboxes it
on wider fullscreen displays. Opening the in-game control panel now switches
to the DOS menu track 14 after the fade-out and restarts the current level's
track after the return fade-in, including after a quickload; the separate
`PAUSEDPIC` pause still stops and resumes gameplay audio. Some control-panel
sound cues use the DOS cursor half-step (`MOVEGUN1SND`) followed eight tics
later by cursor placement (`MOVEGUN2SND`), accept (`SHOOTSND`), cancel
(`ESCPRESSEDSND`), and view-size (`HITWALLSND`) samples. Pause-menu direction
input now repeats after eight tics instead of DOS's 20-tic delay; the
cursor-placement cue still follows each half-step by eight tics.
Opening the in-game help articles now starts `CORNER_MUS` (track 0), as in
`CP_ReadThis`; the other control-panel entries continue to start `MENUSONG`
(track 14). Returning from help restores the current level track.
Other control-panel cues and menu/transition layouts still differ from the DOS
program. Pause-menu direction input now repeats every eight tics using the
same operating-system independent Rust timer as the startup selectors; this
shortens DOS `HandleMenu`'s 20-tic delay for more responsive modern keyboard
input. Both sound panels now offer the DOS sound-effect modes `NONE`, `PC`, and
`ADLIB/SB` as mutually exclusive choices. Digitized playback remains separately
selectable as `OFF` or `ON`; SDL abstracts the old Disney Sound Source and
Sound Blaster hardware backends into the platform audio device. Sound-mode
changes stop the synthesized/PC path and play `SHOOTSND` when switching to PC
or AdLib, while `NONE` is silent; digitized playback remains independently
available as in DOS. Turning digitized audio off immediately stops its active
sample, matching `SD_SetDigiDevice` calling `SD_StopDigitized`. Music-mode
changes play `SHOOTSND` in either direction. Unchanged settings are silent,
and Back uses the `ESCPRESSEDSND` cancellation cue. Re-enabling music restarts
the menu track when `AdLib/Sound Blaster` is selected after music was disabled.
Mouse row
changes in both startup and pause menus now use that same delayed cursor cue.
The main, episode, and difficulty selectors now also draw the original
`C_MOUSELBACKPIC` at the DOS position (112,184).
Accepting a changed view size also restarts the DOS accept cue after applying
the selection.
`.github/workflows/release-packages.yml`
builds the Linux AppImage, Windows ZIP, and universal macOS DMG only after a
version tag is created, then runs `--help` from each packaged executable before
uploading it. These packaged launch checks are configured but have not run yet.
The macOS workflow and SDL2 framework target macOS
11.0, and the application bundle declares the matching
`LSMinimumSystemVersion`. Optional Developer ID signing and App Store Connect
notarization are wired to the secrets documented above. The workflow is
configured but has not been run.

The redistribution question is out of scope: the user confirmed that this
feasibility study will not be published. Original game data remains outside
the default packages.
