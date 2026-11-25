-- Synthetic fixtures, only for isolated pickup_preview.
\set ON_ERROR_STOP on
BEGIN;
INSERT INTO courts (id, name, sport_type, lat, lng, address, price_per_slot, owner_id)
VALUES
('10000000-0000-4000-8000-000000000001', 'Thao Dien Pickleball Club', 'pickleball', 10.8033, 106.7331, '12 Nguyen Van Huong, Thao Dien, Ho Chi Minh City', 360000, :'owner_id'),
('10000000-0000-4000-8000-000000000002', 'Riverside Football Arena', 'mini_football', 10.8001, 106.7252, '24 Tran Nao, An Khanh, Ho Chi Minh City', 600000, :'owner_id'),
('10000000-0000-4000-8000-000000000003', 'District 7 Garden Courts', 'pickleball', 10.7298, 106.7216, '18 Nguyen Luong Bang, Phu My Hung, Ho Chi Minh City', 300000, :'owner_id');
INSERT INTO court_slots (court_id, start_time, end_time)
SELECT c.id, t.start_time, t.start_time + interval '90 minutes'
FROM courts c CROSS JOIN generate_series(0, 3) AS day_offset CROSS JOIN (VALUES (17), (19)) AS hour_slot(hour)
CROSS JOIN LATERAL (SELECT ((now() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date + day_offset + make_interval(hours => hour_slot.hour)) AT TIME ZONE 'Asia/Ho_Chi_Minh' AS start_time) t
WHERE t.start_time > now();
COMMIT;
