ALTER TABLE onboarding_applications ADD COLUMN work_file_url TEXT NOT NULL DEFAULT '';
ALTER TABLE onboarding_applications ADD COLUMN work_file_name TEXT NOT NULL DEFAULT '';
ALTER TABLE onboarding_applications ADD COLUMN verification_items TEXT NOT NULL DEFAULT '[]';
