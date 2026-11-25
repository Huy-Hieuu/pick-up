-- This enum extension must commit before queries/indexes use the new value.
ALTER TYPE payment_status ADD VALUE 'disputed';
