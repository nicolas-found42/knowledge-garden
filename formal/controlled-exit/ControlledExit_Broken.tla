---- MODULE ControlledExit_Broken ----
(* Regression probe: orderly reopen charges the crash budget. *)
EXTENDS Integers

CONSTANTS MaxAudioInterruptions

VARIABLES phase, stopping, helperCount, audioState, interruptions, controlledReceipt,
          budgetViolation

PhaseVal == {"running", "quitting"}
AudioVal == {"idle", "processing", "pending", "failed"}

vars == <<phase, stopping, helperCount, audioState, interruptions, controlledReceipt,
          budgetViolation>>

Init ==
  /\ phase = "running"
  /\ stopping = FALSE
  /\ helperCount = 0
  /\ audioState = "idle"
  /\ interruptions = 0
  /\ controlledReceipt = FALSE
  /\ budgetViolation = FALSE

StartAudioWindow ==
  /\ phase = "running"
  /\ ~stopping
  /\ audioState = "idle"
  /\ audioState' = "processing"
  /\ UNCHANGED <<phase, stopping, helperCount, interruptions, controlledReceipt, budgetViolation>>

ControlledQuit ==
  /\ phase = "running"
  /\ controlledReceipt' = TRUE
  /\ stopping' = TRUE
  /\ helperCount' = 0
  /\ phase' = "quitting"
  /\ UNCHANGED <<audioState, interruptions, budgetViolation>>

ReopenCollection ==
  /\ phase = "quitting"
  /\ stopping = TRUE
  /\ phase' = "running"
  /\ stopping' = FALSE
  /\ IF controlledReceipt
     THEN /\ audioState' = "pending"
          /\ interruptions' = interruptions + 1
     ELSE UNCHANGED <<audioState, interruptions>>
  /\ controlledReceipt' = FALSE
  /\ budgetViolation' =
       IF controlledReceipt /\ audioState = "processing" /\ interruptions' # interruptions
       THEN TRUE
       ELSE budgetViolation

Next ==
  \/ StartAudioWindow
  \/ ControlledQuit
  \/ ReopenCollection

Spec == Init /\ [][Next]_vars

NoOrderlyBudgetViolation == ~budgetViolation

====
