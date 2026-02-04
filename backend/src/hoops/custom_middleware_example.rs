use salvo::prelude::*;

/// 在命中的 handle 处理之后执行逻辑的中间件（后处理）
#[allow(dead_code)]
#[handler]
async fn post_processing_middleware(&self, req: &mut Request, depot: &mut Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    ctrl.call_next(req, depot, res).await;
    // 在命中的 handle 处理之后执行逻辑的中间件（后处理）
}

/// 实现洋葱模型的中间件，具有前置和后置逻辑
#[allow(dead_code)]
#[handler]
async fn onion_model_middleware(&self, req: &mut Request, depot: &mut Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    // 在命中的 handle 处理之前执行逻辑的中间件（预处理）
    ctrl.call_next(req, depot, res).await;
    // 在命中的 handle 处理之后执行逻辑的中间件（后处理）
}

/// 在命中的 handle 处理之前执行逻辑的中间件（预处理）
#[allow(dead_code)]
#[handler]
async fn pre_processing_middleware(&self, req: &mut Request, depot: &mut Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    //  在命中的 handle 处理之前执行逻辑的中间件（预处理）
    ctrl.call_next(req, depot, res).await;
}


/// 跳过剩余处理程序的中间件
#[allow(dead_code)]
#[handler]
async fn skip_handler_middleware(&self, _req: &mut Request, _depot: &mut Depot, _res: &mut Response, ctrl: &mut FlowCtrl) {
    // 跳过剩余处理程序的中间件
    ctrl.skip_rest();
}
