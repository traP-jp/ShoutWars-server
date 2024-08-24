#pragma once

#include <nlohmann/json.hpp>
#include <boost/uuid/uuid.hpp>
#include <chrono>
#include <condition_variable>
#include <shared_mutex>
#include <map>
#include <string>
#include <vector>
#include <functional>
#include <memory>

class room_t {
public:
  // Note that although user_t is mutable, it is not thread-safe.
  class user_t {
  public:
    const boost::uuids::uuid id;

    explicit user_t(const std::string &name);

    friend void to_json(nlohmann::json &j, const user_t &user);

    std::string get_name() const;

    void set_name(const std::string &new_name);

    boost::uuids::uuid get_last_sync_id() const;

    std::chrono::steady_clock::time_point get_last_time() const;

    void update_last(boost::uuids::uuid new_sync_id);

  protected:
    std::string name;
    boost::uuids::uuid last_sync_id;
    std::chrono::steady_clock::time_point last_time;
  };

  class sync_record_t {
  public:
    class event_t {
    public:
      const boost::uuids::uuid id;
      const boost::uuids::uuid from;
      const std::string type;
      const nlohmann::json data;

      explicit event_t(boost::uuids::uuid id, boost::uuids::uuid from, std::string type, nlohmann::json data);

      friend void to_json(nlohmann::json &j, const event_t &event);
    };

    enum class phase_t {
      CREATED = 0, WAITING = 1, SYNCING = 2, SYNCED = 3
    };

    const boost::uuids::uuid id;

    explicit sync_record_t();

    void add_events(
      boost::uuids::uuid from, const std::vector<std::shared_ptr<event_t>> &new_reports,
      const std::vector<std::shared_ptr<event_t>> &new_actions
    );

    std::vector<std::shared_ptr<event_t>> get_reports() const;

    std::vector<std::shared_ptr<event_t>> get_actions() const;

    phase_t get_phase(boost::uuids::uuid user_id);

    bool advance_phase(boost::uuids::uuid user_id, phase_t new_phase);

    phase_t get_max_phase() const;

  protected:
    std::map<boost::uuids::uuid, std::shared_ptr<event_t>> reports;
    std::map<boost::uuids::uuid, std::shared_ptr<event_t>> actions;
    std::map<boost::uuids::uuid, phase_t> users_phase;
    mutable std::shared_mutex record_mutex;
  };

  using logger = std::function<void(const std::string &)>;

  const logger log_error, log_info;

  const boost::uuids::uuid id;
  const std::string version;
  const size_t size;

  explicit room_t(
    std::string version, const user_t &owner, size_t size, logger log_error = [](const std::string &) {},
    logger log_info = [](const std::string &) {}
  );

  void join(std::string version, const user_t &user);

  user_t get_user(boost::uuids::uuid id) const;

  bool has_user(boost::uuids::uuid id) const;

  bool kick(boost::uuids::uuid id);

  size_t kick_expired(std::chrono::milliseconds timeout);

  size_t count_users() const;

  std::vector<boost::uuids::uuid> get_user_ids() const;

  std::vector<user_t> get_users() const;

  user_t get_owner() const;

  bool is_in_lobby() const;

  void start_game();

  bool is_available() const;

  nlohmann::json get_info() const;

  void update_info(const nlohmann::json &new_info);

  std::vector<std::shared_ptr<sync_record_t>> sync(
    boost::uuids::uuid user_id, const std::vector<std::shared_ptr<sync_record_t::event_t>> &reports,
    const std::vector<std::shared_ptr<sync_record_t::event_t>> &actions
  );

  size_t clean_sync_records();

protected:
  static boost::uuids::uuid gen_id();

  std::map<boost::uuids::uuid, user_t> users;
  bool in_lobby = true;
  nlohmann::json info;
  std::map<boost::uuids::uuid, std::shared_ptr<sync_record_t>> sync_records;
  mutable std::shared_mutex room_mutex;
  std::condition_variable_any sync_cv;
};

class room_list_t {
public:
  using logger = std::function<void(const std::string &)>;

  const logger log_error, log_info;

  explicit room_list_t(
    size_t limit, logger log_error = [](const std::string &) {}, logger log_info = [](const std::string &) {}
  );

  std::shared_ptr<room_t> create(const std::string &version, const room_t::user_t &owner, size_t size);

  std::shared_ptr<room_t> get(boost::uuids::uuid id) const;

  bool exists(boost::uuids::uuid id) const;

  bool remove(boost::uuids::uuid id);

  size_t count() const;

  std::vector<std::shared_ptr<room_t>> get_all() const;

  size_t get_limit() const;

  void set_limit(size_t new_limit);

  void clean(std::chrono::milliseconds timeout);

protected:
  std::map<boost::uuids::uuid, std::shared_ptr<room_t>> rooms;
  size_t limit;
  mutable std::shared_mutex rooms_mutex;
};
