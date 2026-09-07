# Oregon preserved continuation history

This archive branch preserves the complete local Git history through 976d6e532552d07c73b901ac0a3eaed414fbc384 without rewriting those commits. It is an archive, not a main integration candidate.

Run `git bundle verify Oregon-continuation-2026-09-07.bundle`, then fetch the bundle's `work/reserve-proof-runner-2026-09-07` branch into a new local recovery branch.

The corresponding published source snapshot is ed4e3b5af5c6e56d0404548866206766f902936d with exact tree 975454b9224584dd20e0b24fd614cb9bb6fc65fb. The publication commit has a different identity; the original commits are retained in the bundle.
