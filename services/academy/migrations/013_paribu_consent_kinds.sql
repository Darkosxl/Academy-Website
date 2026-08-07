-- The Paribu forms arrived: instead of the one placeholder `paribu` slot there are four
-- real documents (aydınlatma + açık rıza, once for the katılımcı and once for the
-- veli/vasi), each its own card and its own upload bucket. See CONSENT_DOCS in model.rs.
--
-- `paribu` itself stays allowed even though it is no longer in CONSENT_DOCS: it was locked
-- from the day it shipped so there should be no rows under it, but a constraint that would
-- fail on a row somebody did manage to upload is not worth the tidiness. It can't be
-- uploaded to any more — valid_consent_kind() only knows what CONSENT_DOCS lists.
--
-- Re-runnable like every migration here: the constraint is dropped by name and recreated,
-- and the name is spelled out rather than left to Postgres' `<table>_<column>_check`
-- default so the next widening has something stable to drop.
alter table consent_docs_exposure_academy
  drop constraint if exists consent_docs_exposure_academy_kind_check;
alter table consent_docs_exposure_academy
  drop constraint if exists consent_docs_exposure_academy_kind_allowed;
alter table consent_docs_exposure_academy
  add constraint consent_docs_exposure_academy_kind_allowed check (kind in (
    'exposure',
    'qnbeyond',
    'paribu',
    'paribu_katilimci_aydinlatma',
    'paribu_katilimci_riza',
    'paribu_veli_aydinlatma',
    'paribu_veli_riza'
  ));

-- The placeholder shipped closed (CONSENT_LOCKED_BY_DEFAULT), so an admin may have a
-- `consent_lock_paribu` row sitting at "1". Nothing reads it now that the kind is gone
-- from CONSENT_DOCS, but leaving it would quietly re-close a future `paribu` card.
delete from app_settings_exposure_academy where key = 'consent_lock_paribu';
