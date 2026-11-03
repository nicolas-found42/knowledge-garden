---- MODULE ControlledExit ----
(*
  Model of explicit Quit vs unmarked crash around helper shutdown and audio
  interruption budgets. Maps to:
  - src-tauri/src/processes.rs (spawn, shutdown, stopping)
  - src-tauri/src/application.rs (record_controlled_shutdown, recover_interrupted_jobs)
  - MAX_AUDIO_INTERRUPTION_ATTEMPTS = 3 in application.rs
*)
EXTENDS Integers

CONSTANTS MaxAudioInterruptions

VARIABLES phase, stopping, helperCount, audioState, interruptions, controlledReceipt,
          budgetViolation

PhaseVal == {"running", "quitting"}
AudioVal == {"idle", "processing", "pending", "failed"}

vars == <<phase, stopping, helperCount, audioState, interruptions, controlledReceipt,
          budgetViolation>>

TypeOK ==
  /\ phase \in PhaseVal
  /\ stopping \in BOOLEAN
  /\ helperCount \in 0..3
  /\ audioState \in AudioVal
  /\ interruptions \in 0..MaxAudioInterruptions
  /\ controlledReceipt \in BOOLEAN
  /\ budgetViolation \in BOOLEAN

Init ==
  /\ phase = "running"
  /\ stopping = FALSE
  /\ helperCount = 0
  /\ audioState = "idle"
  /\ interruptions = 0
  /\ controlledReceipt = FALSE
  /\ budgetViolation = FALSE

SpawnHelper ==
  /\ ~stopping
  /\ helperCount < 3
  /\ helperCount' = helperCount + 1
  /\ UNCHANGED <<phase, stopping, audioState, interruptions, controlledReceipt, budgetViolation>>

StartAudioWindow ==
  /\ phase = "running"
  /\ ~stopping
  /\ audioState = "idle"
  /\ audioState' = "processing"
  /\ UNCHANGED <<phase, stopping, helperCount, interruptions, controlledReceipt, budgetViolation>>

(* record_controlled_shutdown + processes::shutdown on Quit *)
ControlledQuit ==
  /\ phase = "running"
  /\ controlledReceipt' = TRUE
  /\ stopping' = TRUE
  /\ helperCount' = 0
  /\ phase' = "quitting"
  /\ UNCHANGED <<audioState, interruptions, budgetViolation>>

(* Process kill without a controlled-exit receipt (crash or SIGKILL) *)
UnmarkedQuit ==
  /\ phase = "running"
  /\ controlledReceipt' = FALSE
  /\ stopping' = TRUE
  /\ helperCount' = 0
  /\ phase' = "quitting"
  /\ UNCHANGED <<audioState, interruptions, budgetViolation>>

(* Application::open after exclusive lock; recover_interrupted_jobs(orderly) *)
ReopenCollection ==
  /\ phase = "quitting"
  /\ stopping = TRUE
  /\ phase' = "running"
  /\ stopping' = FALSE
  /\ IF controlledReceipt
     THEN /\ IF audioState = "processing"
               THEN /\ audioState' = "pending"
                    /\ interruptions' = interruptions
               ELSE /\ audioState' = audioState
                    /\ interruptions' = interruptions
     ELSE /\ IF audioState = "processing"
               THEN /\ interruptions' = IF interruptions >= MaxAudioInterruptions
                                         THEN interruptions
                                         ELSE interruptions + 1
                    /\ audioState' =
                         IF interruptions + 1 >= MaxAudioInterruptions
                         THEN "failed"
                         ELSE "pending"
               ELSE /\ audioState' = audioState
                    /\ interruptions' = interruptions
  /\ controlledReceipt' = FALSE
  /\ budgetViolation' =
       IF controlledReceipt /\ audioState = "processing" /\ interruptions' # interruptions
       THEN TRUE
       ELSE budgetViolation
  /\ helperCount' = helperCount

Next ==
  \/ SpawnHelper
  \/ StartAudioWindow
  \/ ControlledQuit
  \/ UnmarkedQuit
  \/ ReopenCollection

Spec == Init /\ [][Next]_vars

(* processes.rs: after shutdown(), helpers are reaped and spawn is rejected *)
NoHelpersWhileStopping == stopping => helperCount = 0

(* recover_interrupted_jobs: orderly Quit must not charge the crash budget *)
NoOrderlyBudgetViolation == ~budgetViolation

InterruptionCap == interruptions <= MaxAudioInterruptions

====
