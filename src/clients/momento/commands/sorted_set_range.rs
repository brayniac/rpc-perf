use super::*;

use ::momento::cache::{SortedSetFetchByRankRequest, SortedSetFetchByScoreRequest, SortedSetOrder};

/// Performs a range query on a sorted set, returning the specified range of
/// elements. Supports selecting a range of keys by index (rank).
pub async fn sorted_set_range(
    client: &mut CacheClient,
    config: &Config,
    cache_name: &str,
    request: workload::client::SortedSetRange,
) -> std::result::Result<(), ResponseError> {
    SORTED_SET_RANGE.increment();

    let result = if !request.by_score {
        let mut r = SortedSetFetchByRankRequest::new(cache_name, &*request.key)
            .order(SortedSetOrder::Ascending);

        if let Some(start) = request.start {
            r = r.start_rank(start);
        }

        if let Some(end) = request.end {
            r = r.end_rank(end);
        }

        timeout(
            config.client().unwrap().request_timeout(),
            client.send_request(r),
        )
        .await
    } else {
        let mut r = SortedSetFetchByScoreRequest::new(cache_name, &*request.key)
            .order(SortedSetOrder::Ascending);

        if let Some(start) = request.start {
            r = r.min_score(start.into());
        }

        if let Some(end) = request.end {
            r = r.max_score(end.into());
        }

        timeout(
            config.client().unwrap().request_timeout(),
            client.send_request(r),
        )
        .await
    };

    record_result!(result, SORTED_SET_RANGE)
}
