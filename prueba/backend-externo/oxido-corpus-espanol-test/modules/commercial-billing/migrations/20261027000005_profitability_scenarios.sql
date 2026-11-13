CREATE TABLE catalog_profitability_scenarios (
    plan_id UUID NOT NULL,
    plan_version INTEGER NOT NULL,
    locale TEXT NOT NULL,
    currency CHAR(3) NOT NULL,
    provider_expected_minor_per_credit BIGINT NOT NULL CHECK (provider_expected_minor_per_credit >= 0),
    provider_stress_minor_per_credit BIGINT NOT NULL CHECK (provider_stress_minor_per_credit >= provider_expected_minor_per_credit),
    operating_minor_per_credit BIGINT NOT NULL CHECK (operating_minor_per_credit >= 0),
    payment_fee_basis_points INTEGER NOT NULL CHECK (payment_fee_basis_points BETWEEN 0 AND 5000),
    payment_fixed_minor BIGINT NOT NULL CHECK (payment_fixed_minor >= 0),
    target_margin_basis_points INTEGER NOT NULL CHECK (target_margin_basis_points BETWEEN 0 AND 9000),
    expected_margin_basis_points INTEGER NOT NULL,
    stress_margin_basis_points INTEGER NOT NULL,
    evaluated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (plan_id, plan_version, locale, currency),
    FOREIGN KEY (plan_id, plan_version) REFERENCES catalog_plans(plan_id, version)
);
