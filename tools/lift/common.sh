# Paths shared by the lift scripts. Override any of them in the environment.
# LIFT_DATA holds what the lift is made from: coverage files, driver dumps,
# extra block starts. They come from the game: keep them out of git.
REPO=$(cd "$(dirname "$0")/../.." && pwd)
LIFT_DATA=${LIFT_DATA:-$REPO/../dune-lift}
GAME=${GAME:-$REPO/../Cryogenic-Web}
LST=${LST:-$REPO/../Cryogenic/doc/DNCDPRG.lst}
HAND_TARGET=${HAND_TARGET:-$REPO/target-hand}     # with the interpreter
NOINT_TARGET=${NOINT_TARGET:-$REPO/target-noint}  # without it
RUN_HAND=$HAND_TARGET/release/dune-run
RUN_NOINT=$NOINT_TARGET/release/dune-run
# Compiling the generated code takes a lot of memory: few jobs at once.
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
