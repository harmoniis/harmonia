(* Lambdoma.v — machine-checked proof of the lambdoma-reduction soundness invariants.
 *
 * This is the STATIC counterpart to the runtime contract `lambdoma-recall-coherent`
 * (src/core/formal-verification.lisp): that contract checks, over recall's ACTUAL output,
 * that the lambdoma-reduced result is BOUNDED (|output| <= limit) and a SUBSET of the
 * candidates (no fabrication). Here we prove those two invariants for ALL k and ALL
 * candidate lists, machine-checked by Rocq/Coq — the Software Foundations standard.
 *
 * %lambdoma-select (src/memory/store/operations.lisp) sorts candidates by field resonance
 * and keeps the top-k. The soundness invariants we depend on are independent of WHICH order
 * the sort imposes — they hold for "take the first k of any candidate list" — so the model
 * abstracts the resonance sort and proves the invariants of the reduction itself.
 *
 * Build:  rocq compile proofs/Lambdoma.v   (or: coqc proofs/Lambdoma.v)
 *)

From Stdlib Require Import List.
From Stdlib Require Import Lia.
Import ListNotations.

(* The lambdoma reduction, modeled: keep the top-k of the (resonance-ordered) candidates. *)
Definition lambdoma_select {A : Type} (k : nat) (candidates : list A) : list A :=
  firstn k candidates.

(* ── Invariant 1: BOUNDED. The reduction never returns more than k elements. ──
   This is exactly the bound the runtime contract asserts dynamically (:BOUNDED T);
   proven here for every k and every candidate list. *)
Theorem lambdoma_bounded :
  forall (A : Type) (k : nat) (c : list A),
    length (lambdoma_select k c) <= k.
Proof.
  intros A k. unfold lambdoma_select.
  induction k as [| k IH]; intros c.
  - simpl. lia.
  - destruct c as [| a c']; simpl.
    + lia.
    + specialize (IH c'). lia.
Qed.

(* ── Invariant 2: SUBSET (no fabrication). Every selected element was a candidate. ──
   The runtime contract checks the output entries are GENUINE; this proves the reduction
   itself can only ever return things that were in the input — it cannot invent results. *)
Theorem lambdoma_subset :
  forall (A : Type) (k : nat) (c : list A) (x : A),
    In x (lambdoma_select k c) -> In x c.
Proof.
  intros A k. unfold lambdoma_select.
  induction k as [| k IH]; intros c x H.
  - simpl in H. contradiction.
  - destruct c as [| a c']; simpl in H.
    + contradiction.
    + destruct H as [H | H].
      * left. exact H.
      * right. apply (IH c' x H).
Qed.

(* ── Corollary: the reduction preserves the prefix exactly when k covers the input. ──
   When asked for at least as many as exist, lambdoma returns the whole candidate set —
   it never silently drops a candidate it had room for. *)
Theorem lambdoma_total_when_room :
  forall (A : Type) (k : nat) (c : list A),
    length c <= k -> lambdoma_select k c = c.
Proof.
  intros A k. unfold lambdoma_select.
  induction k as [| k IH]; intros c H.
  - destruct c as [| a c'].
    + reflexivity.
    + simpl in H. lia.
  - destruct c as [| a c'].
    + reflexivity.
    + simpl. f_equal. apply IH. simpl in H. lia.
Qed.
