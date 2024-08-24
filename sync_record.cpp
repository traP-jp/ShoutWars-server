#include "sync_record.hpp"

#include "errors.hpp"
#include <ranges>
#include <utility>

using namespace std;
using namespace boost::uuids;

using json = nlohmann::json;

uuid sync_record_t::gen_id() {
  thread_local time_generator_v7 gen;
  return gen();
}

// room sync

sync_record_t::sync_record_t() : id(gen_id()) {}

void sync_record_t::add_events(
  const uuid from, const vector<shared_ptr<event_t>> &new_reports, const vector<shared_ptr<event_t>> &new_actions
) {
  lock_guard lock(record_mutex);
  if (users_phase[from] > phase_t::CREATED) throw bad_request_error("Record already synced.");
  for (const shared_ptr<event_t> &report: new_reports) {
    if (report->from != from) throw bad_request_error("Invalid report from.");
    reports[report->id] = report;
  }
  for (const shared_ptr<event_t> &action: new_actions) {
    if (action->from != from) throw bad_request_error("Invalid action from.");
    actions[action->id] = action;
  }
  users_phase[from] = phase_t::WAITING;
}

vector<shared_ptr<sync_record_t::event_t>> sync_record_t::get_reports() const {
  shared_lock lock(record_mutex);
  const auto view = reports | views::transform(
                      [](const pair<uuid, shared_ptr<event_t>> &report) { return report.second; }
                    );
  return move(vector(view.begin(), view.end()));
}

vector<shared_ptr<sync_record_t::event_t>> sync_record_t::get_actions() const {
  shared_lock lock(record_mutex);
  const auto view = actions | views::transform(
                      [](const pair<uuid, shared_ptr<event_t>> &action) { return action.second; }
                    );
  return move(vector(view.begin(), view.end()));
}

sync_record_t::phase_t sync_record_t::get_phase(const uuid user_id) {
  lock_guard lock(record_mutex);
  return users_phase[user_id];
}

bool sync_record_t::advance_phase(const uuid user_id, const phase_t new_phase) {
  lock_guard lock(record_mutex);
  if (new_phase <= users_phase[user_id]) return false;
  users_phase[user_id] = new_phase;
  return true;
}

sync_record_t::phase_t sync_record_t::get_max_phase() const {
  shared_lock lock(record_mutex);
  return ranges::max(users_phase | views::values);
}

// room sync event

sync_record_t::event_t::event_t(const uuid id, const uuid from, string type, json data)
  : id(id), from(from), type(move(type)), data(move(data)) {}

void to_json(json &j, const sync_record_t::event_t &event) {
  j = {
    { "id", to_string(event.id) },
    { "from", to_string(event.from) },
    { "type", event.type },
    { "event", event.data }
  };
}
