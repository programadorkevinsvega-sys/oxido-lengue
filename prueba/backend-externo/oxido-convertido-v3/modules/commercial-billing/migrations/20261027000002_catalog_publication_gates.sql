ALTER TABLE catalog_plans
    ADD COLUMN commercial_gate_reference TEXT,
    ADD COLUMN tax_gate_reference TEXT;

ALTER TABLE catalog_plans
    ADD CONSTRAINT catalog_plans_published_gates_check
    CHECK (lifecycle <> 'published' OR
           (profitability_evidence IS NOT NULL AND approved_by_operator IS NOT NULL
            AND approved_at IS NOT NULL AND commercial_gate_reference IS NOT NULL
            AND tax_gate_reference IS NOT NULL));
