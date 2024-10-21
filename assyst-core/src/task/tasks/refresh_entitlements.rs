use std::collections::HashMap;

use assyst_common::err;
use assyst_common::macros::handle_log;
use assyst_database::model::active_guild_premium_entitlement::ActiveGuildPremiumEntitlement;
use tracing::info;

use crate::assyst::ThreadSafeAssyst;

pub async fn refresh_entitlements(assyst: ThreadSafeAssyst) {
    let entitlements = assyst.entitlements.lock().unwrap().clone();

    let additional = match assyst.http_client.entitlements(assyst.application_id).await {
        Ok(x) => match x.model().await {
            Ok(e) => e,
            Err(e) => {
                err!("Failed to get potential new entitlements: {e:?}");
                vec![]
            },
        },
        Err(e) => {
            err!("Failed to get potential new entitlements: {e:?}");
            vec![]
        },
    };

    println!("{:?}", additional.iter().map(|x| x.id).collect::<Vec<_>>());

    for a in additional {
        if !assyst.entitlements.lock().unwrap().contains_key(&(a.id.get() as i64)) {
            let active = match ActiveGuildPremiumEntitlement::try_from(a) {
                Ok(a) => a,
                Err(e) => {
                    err!("Error processing new entitlement: {e:?}");
                    continue;
                },
            };

            if let Err(e) = active.set(&assyst.database_handler).await {
                err!("Error adding new entitlement for ID {}: {e:?}", active.entitlement_id);
            };
            handle_log(format!("New entitlement! Guild: {}", active.guild_id));

            assyst
                .entitlements
                .lock()
                .unwrap()
                .insert(active.entitlement_id, active);
        }
    }

    let db_entitlements = ActiveGuildPremiumEntitlement::get_all(&assyst.database_handler)
        .await
        .ok()
        .unwrap_or(HashMap::new());

    // remove entitlements from the db that are not in the rest response
    for entitlement in db_entitlements.values() {
        if !entitlements.contains_key(&entitlement.entitlement_id) {
            assyst.entitlements.lock().unwrap().remove(&entitlement.entitlement_id);
            info!(
                "Removed expired entitlement {} (guild {})",
                entitlement.entitlement_id, entitlement.guild_id
            );
            if let Err(e) =
                ActiveGuildPremiumEntitlement::delete(&assyst.database_handler, entitlement.entitlement_id).await
            {
                err!(
                    "Error deleting existing entitlement {}: {e:?}",
                    entitlement.entitlement_id
                );
            }
        }
    }
}
