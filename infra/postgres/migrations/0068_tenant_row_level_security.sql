-- Defence in depth for shared-schema tenant isolation.
--
-- The application still binds tenant_id in every query. These policies add a
-- database boundary when the runtime connects through a non-owner role and sets
-- `app.tenant_id` for the transaction. Table owners retain PostgreSQL's normal
-- RLS bypass so migrations and the current owner-role deployment remain
-- backwards compatible; production should use a dedicated non-owner app role.
DO $$
DECLARE
    tenant_table record;
BEGIN
    FOR tenant_table IN
        SELECT DISTINCT c.table_schema, c.table_name
        FROM information_schema.columns c
        JOIN information_schema.tables t
          ON t.table_schema = c.table_schema
         AND t.table_name = c.table_name
        WHERE c.table_schema = 'public'
          AND c.column_name = 'tenant_id'
          AND t.table_type = 'BASE TABLE'
    LOOP
        EXECUTE format(
            'ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY',
            tenant_table.table_schema,
            tenant_table.table_name
        );

        IF NOT EXISTS (
            SELECT 1
            FROM pg_policies
            WHERE schemaname = tenant_table.table_schema
              AND tablename = tenant_table.table_name
              AND policyname = 'tenant_isolation'
        ) THEN
            EXECUTE format(
                'CREATE POLICY tenant_isolation ON %I.%I '
                'USING (tenant_id::text = NULLIF(current_setting(''app.tenant_id'', true), '''')) '
                'WITH CHECK (tenant_id::text = NULLIF(current_setting(''app.tenant_id'', true), ''''))',
                tenant_table.table_schema,
                tenant_table.table_name
            );
        END IF;
    END LOOP;
END
$$;
