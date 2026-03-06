use rust_embed::RustEmbed;
use salvo::prelude::*;
use salvo::serve_static::{static_embed, EmbeddedFileExt};

mod auth;
mod demo;
mod user;
mod memory;
mod memory_storage;
mod memory_search;
mod knowledge_graph;
mod multimodal;

use crate::{config, hoops};

#[derive(RustEmbed)]
#[folder = "assets"]
struct Assets;

pub fn root() -> Router {
    let favicon = Assets::get("favicon.ico")
        .expect("favicon not found")
        .into_handler();
    let router = Router::new()
        .hoop(Logger::new())
        .get(demo::hello)
        .push(Router::with_path("login").get(auth::login_page))
        .push(Router::with_path("register").post(auth::register))
        .push(Router::with_path("users").get(user::list_page))
        .push(
            Router::with_path("api")
                .push(
                    Router::with_path("login")
                        .post(auth::post_login)
                        .push(
                            Router::with_path("account")
                                .post(auth::post_login_with_token)
                                .get(auth::get_login_with_token)
                        )
                )
                .push(
                    Router::with_path("currentUser")
                        .hoop(hoops::auth_hoop(&config::get().jwt))
                        .get(auth::get_current_user)
                )
                .push(
                    Router::with_path("users")
                        .hoop(hoops::auth_hoop(&config::get().jwt))
                        .get(user::list_users)
                        .post(user::create_user)
                        .push(
                            Router::with_path("{user_id}")
                                .put(user::update_user)
                                .delete(user::delete_user),
                        ),
                )
                .push(
                    Router::with_path("v1/memory")
                        // 限流：每分钟 100 次请求
                        .hoop(hoops::rate_limit_hoop(100, 60))
                        .push(
                            Router::with_path("adaptive")
                                .post(memory::select_memory_config)
                                .push(Router::with_path("trace").post(memory::select_memory_config_trace))
                                .get(memory::get_memory_status),
                        )
                        .push(Router::with_path("traces").get(memory::get_decision_traces))
                        .push(
                            Router::with_path("analyzer")
                                .push(
                                    Router::with_path("task-characteristics")
                                        .post(memory::analyze_task_characteristics),
                                )
                                .push(
                                    Router::with_path("batch-characteristics")
                                        .post(memory::batch_analyze_characteristics),
                                ),
                        )
                        .push(
                            Router::with_path("predictor")
                                .push(
                                    Router::with_path("performance")
                                        .post(memory::predict_performance),
                                )
                                .push(
                                    Router::with_path("baselines")
                                        .get(memory::get_baselines),
                                ),
                        )
                        .push(
                            Router::with_path("monitor")
                                .push(
                                    Router::with_path("resources")
                                        .get(memory::get_resources),
                                )
                                .push(
                                    Router::with_path("cost-benefit")
                                        .post(memory::calculate_cost_benefit),
                                )
                                .push(
                                    Router::with_path("optimize")
                                        .post(memory::optimize),
                                ),
                        )
                        .push(
                            Router::with_path("weights")
                                .push(
                                    Router::with_path("adjust")
                                        .post(memory::adjust_weights),
                                )
                                .push(
                                    Router::with_path("history")
                                        .get(memory::get_weight_history),
                                ),
                        )
                        .push(
                            Router::with_path("health")
                                .get(memory::health_check),
                        )
                       .push(
                           Router::with_path("config")
                               .get(memory::get_config),
                       )
                       .push(
                           Router::with_path("configs")
                               .hoop(hoops::auth_hoop(&config::get().jwt))
                               .get(memory::list_memory_configs)
                               .post(memory::create_memory_config)
                               .push(
                                   Router::with_path("{config_id}")
                                       .get(memory::get_memory_config)
                                       .put(memory::update_memory_config)
                                       .delete(memory::delete_memory_config),
                               ),
                       )
                       .push(
                           Router::with_path("storage")
                               .push(
                                   Router::with_path("sessions")
                                       .get(memory_storage::list_sessions),
                               )
                               .push(
                                   Router::with_path("stm")
                                       .post(memory_storage::store_stm)
                                       .push(
                                           Router::with_path("{session_id}")
                                               .get(memory_storage::get_session_messages),
                                       ),
                               )
                               .push(
                                   Router::with_path("ltm")
                                       .post(memory_storage::store_ltm),
                               )
                               .push(
                                   Router::with_path("transfer")
                                       .post(memory_storage::transfer_stm_to_ltm),
                               )
                               .push(
                                   Router::with_path("batch-ltm")
                                       .post(memory_storage::batch_store_ltm),
                               ),
                       )
                       .push(
                           Router::with_path("search")
                               .push(
                                   Router::with_path("stm")
                                       .post(memory_search::search_stm),
                               )
                               .push(
                                   Router::with_path("ltm")
                                       .get(memory_search::list_ltm_entries)
                                       .post(memory_search::search_ltm)
                                       .push(
                                           Router::with_path("{entry_id}")
                                               .get(memory_search::get_ltm_entry),
                                       ),
                               )
                               .push(
                                   Router::with_path("hybrid")
                                       .post(memory_search::hybrid_search),
                               )
                               .push(
                                   Router::with_path("entity")
                                       .post(memory_search::search_by_entity),
                               ),
                       ),
                )
                // Knowledge Graph API
                .push(
                    Router::with_path("kg")
                        .push(
                            Router::with_path("entities")
                                .get(knowledge_graph::list_entities)
                                .post(knowledge_graph::create_entity)
                        )
                        .push(
                            Router::with_path("entities/by-name/{name}")
                                .get(knowledge_graph::get_entity_by_name)
                        )
                        .push(
                            Router::with_path("entities/{entity_id}/related")
                                .get(knowledge_graph::get_related_entities)
                        )
                        .push(
                            Router::with_path("relations")
                                .post(knowledge_graph::create_relation)
                        )
                        .push(
                            Router::with_path("search")
                                .post(knowledge_graph::search_by_entity)
                        ),
                )
                // Multimodal API
                .push(
                    Router::with_path("mm")
                        .push(
                            Router::with_path("store")
                                .post(multimodal::store_mm)
                        )
                        .push(
                            Router::with_path("entry/{entry_id}")
                                .get(multimodal::get_mm)
                        )
                        .push(
                            Router::with_path("session/{session_id}")
                                .get(multimodal::get_session_mm)
                        )
                        .push(
                            Router::with_path("modality/{modality_type}")
                                .get(multimodal::get_by_modality)
                        )
                        .push(
                            Router::with_path("list")
                                .get(multimodal::list_mm)
                        ),
                )
        )
        .push(Router::with_path("favicon.ico").get(favicon))
        .push(Router::with_path("assets/{**rest}").get(static_embed::<Assets>()));
    let doc = OpenApi::new("salvo web api", "0.0.1").merge_router(&router);
    router
        .unshift(doc.into_router("/api-doc/openapi.json"))
        .unshift(Scalar::new("/api-doc/openapi.json").into_router("scalar"))
}
