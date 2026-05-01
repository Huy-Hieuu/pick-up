-- Haversine distance helper for clean SQL queries.
-- Usage: SELECT haversine_km(lat, lng, 10.762622, 106.660172) FROM courts;
CREATE OR REPLACE FUNCTION haversine_km(
    lat1 double precision,
    lng1 double precision,
    lat2 double precision,
    lng2 double precision
) RETURNS double precision
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
    SELECT 6371 * acos(
        GREATEST(-1.0, LEAST(1.0,
            cos(radians(lat1)) * cos(radians(lat2)) *
            cos(radians(lng2) - radians(lng1)) +
            sin(radians(lat1)) * sin(radians(lat2))
        ))
    );
$$;
