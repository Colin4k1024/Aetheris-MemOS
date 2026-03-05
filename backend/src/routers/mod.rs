use rust_embed::RustEmbed;
use salvo::prelude::*;
use salvo::serve_static::{static_embed, EmbeddedFileExt};

mod auth;
mod demo;
mod user;
mod memory;
mod memory_storage;
mod memory_search;

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
                ),
        )
        .push(Router::with_path("favicon.ico").get(favicon))
        .push(Router::with_path("assets/{**rest}").get(static_embed::<Assets>()));
    let doc = OpenApi::new("salvo web api", "0.0.1").merge_router(&router);
    router
        .unshift(doc.into_router("/api-doc/openapi.json"))
        .unshift(Scalar::new("/api-doc/openapi.json").into_router("scalar"))
}
