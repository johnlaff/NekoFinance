-- Estado LOCAL do check-out: quando/por qual aparelho foi a ÚLTIMA vez que este aparelho puxou o
-- snapshot remoto (o contraponto de `last_checkin_at`/`last_checkin_device_id`, já existentes).
-- `last_checkout_device_id` guarda o `device_id` do manifest remoto BAIXADO, nunca o deste
-- aparelho — é "de qual aparelho veio o que eu recebi", o mesmo formato que `driveCheckinLabel`
-- já usa para "por qual aparelho" do lado do check-in.
ALTER TABLE snapshot_state ADD COLUMN last_checkout_at TEXT;
ALTER TABLE snapshot_state ADD COLUMN last_checkout_device_id TEXT;
