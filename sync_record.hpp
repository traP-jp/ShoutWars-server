#pragma once

#include <nlohmann/json.hpp>
#include <boost/uuid.hpp>
#include <shared_mutex>
#include <map>
#include <string>
#include <vector>
#include <memory>

class sync_record_t {
public:
  class event_t {
  public:
    const boost::uuids::uuid id;
    const boost::uuids::uuid from;
    const std::string type;
    const nlohmann::json data;

    [[nodiscard]] explicit event_t(
      boost::uuids::uuid id, boost::uuids::uuid from, std::string type, nlohmann::json data
    );

    friend void to_json(nlohmann::json &j, const event_t &event);
  };

  enum class phase_t { CREATED = 0, WAITING = 1, SYNCING = 2, SYNCED = 3 };

  const boost::uuids::uuid id;

  [[nodiscard]] explicit sync_record_t();

  void add_events(
    boost::uuids::uuid from, const std::vector<std::shared_ptr<event_t>> &new_reports,
    const std::vector<std::shared_ptr<event_t>> &new_actions
  );

  [[nodiscard]] std::vector<std::shared_ptr<event_t>> get_reports() const;

  [[nodiscard]] std::vector<std::shared_ptr<event_t>> get_actions() const;

  [[nodiscard]] phase_t get_phase(boost::uuids::uuid user_id);

  bool advance_phase(boost::uuids::uuid user_id, phase_t new_phase);

  [[nodiscard]] phase_t get_max_phase() const;

protected:
  static boost::uuids::uuid gen_id();

  mutable std::shared_mutex record_mutex;
  std::map<boost::uuids::uuid, std::shared_ptr<event_t>> reports;
  std::map<boost::uuids::uuid, std::shared_ptr<event_t>> actions;
  std::map<boost::uuids::uuid, phase_t> users_phase;
};
